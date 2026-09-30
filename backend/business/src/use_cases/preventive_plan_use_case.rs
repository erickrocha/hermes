use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::{PreventiveControlType, PreventiveStatus};
use crate::domain::preventive_plan::{PreventivePlan, PreventivePlanEntityMapper};
use crate::gateway::preventive_plan_gateway::PreventivePlanGateway;
use crate::domain::enums::{WorkOrderOrigin, WorkOrderStatus};
use crate::domain::work_order::{WorkOrder, WorkOrderEntityMapper};
use crate::domain::enums::WorkOrderItemStatus;
use crate::domain::preventive_plan_extension::{PreventivePlanExtension, PreventivePlanExtensionEntityMapper};
use crate::domain::work_order_item::{WorkOrderItem, WorkOrderItemEntityMapper};
use crate::gateway::preventive_plan_extension_gateway::PreventivePlanExtensionGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use crate::gateway::work_order_item_gateway::WorkOrderItemGateway;
use crate::gateway::work_order_gateway::WorkOrderGateway;
use chrono::Duration;
use sea_orm::DbErr;

pub const PLAN_NAME_REQUIRED: &str = "A preventive plan needs a name";
pub const INTERVAL_REQUIRED: &str = "The control type needs its matching interval (kilometres, days, or both)";
pub const VEHICLE_NOT_FOUND: &str = "Vehicle not found";
pub const DUPLICATE_PLAN: &str = "This vehicle already has a preventive plan with this name";
pub const PREVENTIVE_PLAN_NOT_FOUND: &str = "Preventive plan not found";
pub const EXTENSION_GRANT_INVALID: &str = "The granted kilometres must be positive and within the maximum extension";
pub const EXTENSION_DESCRIPTION_REQUIRED: &str = "An extension needs a description of what was inspected";
pub const EXTENSION_ITEM_NOT_LINKED: &str = "The work-order item is not linked to this preventive plan";
pub const EXTENSION_ITEM_NOT_PENDING: &str = "Only a pending work-order item can be resolved by an extension";
pub const EXTENSION_ITEM_NOT_FOUND: &str = "Work order item not found";
/// `TRM-313`'s `[TC]`: the most one extension may grant.
pub const MAX_EXTENSION_KM: f64 = 50_000.0;
pub const ORIGIN_ORDER_NOT_ON_VEHICLE: &str = "The originating work order does not belong to this plan's vehicle";
pub const PLAN_NOT_DUE: &str = "This preventive plan is not yet due";

/// `TRM-303`'s own `TC`: within 1,000 km or 15 days of the next service is
/// "requiring attention," not yet overdue.
pub const ATTENTION_KM_MARGIN: f64 = 1_000.0;
pub const ATTENTION_DAYS_MARGIN: i64 = 15;

pub struct PreventivePlanUseCase {
    gateway: PreventivePlanGateway,
    vehicles: VehicleGateway,
    work_orders: WorkOrderGateway,
    items: WorkOrderItemGateway,
    extensions: PreventivePlanExtensionGateway,
}

impl PreventivePlanUseCase {
    pub fn new(
        gateway: PreventivePlanGateway,
        vehicles: VehicleGateway,
        work_orders: WorkOrderGateway,
        items: WorkOrderItemGateway,
        extensions: PreventivePlanExtensionGateway,
    ) -> Self {
        Self { gateway, vehicles, work_orders, items, extensions }
    }

    /// `TRM-330`: edits a plan's control type, intervals and base (last
    /// service km/date). The name and vehicle are the plan's business key
    /// (`TRM-300`) and never change. Changing the base starts a new cycle:
    /// any active extension ends (its entries are kept) and the plan's last
    /// work order becomes the one `edited` names as the origin, or none.
    /// Without a base change `edited.last_work_order_id` is ignored.
    pub async fn update(&self, edited: PreventivePlan) -> Result<PreventivePlan, BusinessError> {
        let uuid = edited.uuid.clone().unwrap_or_default();
        let (current, _) = self.find_by_uuid(uuid).await?;
        check_intervals(&edited)?;

        let new_cycle = edited.last_service_km != current.last_service_km
            || edited.last_service_date != current.last_service_date;
        if new_cycle && let Some(order_id) = edited.last_work_order_id {
            let order = self.work_orders.find_by_id(order_id).await.map_err(database_error)?;
            if !order.is_some_and(|o| o.vehicle_id == current.vehicle_id) {
                return Err(BusinessError::new(ORIGIN_ORDER_NOT_ON_VEHICLE.to_string()));
            }
        }
        let plan = PreventivePlan {
            last_work_order_id: if new_cycle { edited.last_work_order_id } else { current.last_work_order_id },
            control_type: edited.control_type,
            interval_km: edited.interval_km,
            interval_days: edited.interval_days,
            last_service_km: edited.last_service_km,
            last_service_date: edited.last_service_date,
            extension_limit_km: if new_cycle { None } else { current.extension_limit_km },
            ..current
        };
        let entity = self.gateway.persist(plan).await.map_err(database_error)?;
        Ok(PreventivePlanEntityMapper::from_active_model(entity))
    }

    /// `TRM-312…320`: extends a plan after a technical inspection, granting
    /// extra kilometres without a part replacement. The plan's next-service
    /// limit becomes inspection km + granted km (`TRM-316`), the previous
    /// baseline is kept on an auditable entry (`TRM-317`), the inspected
    /// item is resolved with a narrative (`TRM-318`) and its order is left
    /// partially resolved for conferral (`TRM-319`) -- never concluded, and
    /// never renewing the cycle (`TRM-311`, see `WorkOrderUseCase`). The
    /// inspection odometer defaults to the vehicle's current one (`TRM-332`).
    pub async fn extend(
        &self,
        plan_uuid: String,
        item_uuid: String,
        inspection_km: Option<f64>,
        granted_km: f64,
        description: String,
    ) -> Result<PreventivePlanExtension, BusinessError> {
        if !(granted_km > 0.0 && granted_km <= MAX_EXTENSION_KM) {
            return Err(BusinessError::new(EXTENSION_GRANT_INVALID.to_string()));
        }
        let description = description.trim().to_string();
        if description.is_empty() {
            return Err(BusinessError::new(EXTENSION_DESCRIPTION_REQUIRED.to_string()));
        }

        let (plan, _) = self.find_by_uuid(plan_uuid).await?;
        let item = self
            .items
            .find_by_uuid(item_uuid)
            .await
            .map_err(database_error)?
            .map(WorkOrderItemEntityMapper::from_model)
            .ok_or_else(|| BusinessError::new(EXTENSION_ITEM_NOT_FOUND.to_string()))?;
        // `TRM-315`: linked to this plan, and the order is the plan's vehicle's.
        let order = self
            .work_orders
            .find_by_id(item.work_order_id)
            .await
            .map_err(database_error)?;
        let Some(order) = order.filter(|o| o.vehicle_id == plan.vehicle_id) else {
            return Err(BusinessError::new(EXTENSION_ITEM_NOT_LINKED.to_string()));
        };
        if item.preventive_plan_id != plan.id {
            return Err(BusinessError::new(EXTENSION_ITEM_NOT_LINKED.to_string()));
        }
        if item.status != WorkOrderItemStatus::Pending {
            return Err(BusinessError::new(EXTENSION_ITEM_NOT_PENDING.to_string()));
        }

        let inspection_km = match inspection_km {
            Some(km) => km,
            None => self
                .vehicles
                .find_by_id(plan.vehicle_id)
                .await
                .map_err(database_error)?
                .and_then(|v| v.odometer_km)
                .unwrap_or(order.odometer_km),
        };
        if inspection_km < 0.0 {
            return Err(BusinessError::new(EXTENSION_GRANT_INVALID.to_string()));
        }
        let limit = inspection_km + granted_km;

        let entry = PreventivePlanExtension {
            id: None,
            uuid: None,
            tenant_id: plan.tenant_id,
            preventive_plan_id: plan.id.unwrap_or_default(),
            work_order_item_id: item.id.unwrap_or_default(),
            inspection_km,
            granted_km,
            resulting_limit_km: limit,
            description: description.clone(),
            previous_last_service_km: plan.last_service_km,
            previous_last_service_date: plan.last_service_date,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        };
        let saved = self.extensions.persist(entry).await.map_err(database_error)?;

        self.gateway
            .persist(PreventivePlan { extension_limit_km: Some(limit), ..plan })
            .await
            .map_err(database_error)?;

        self.items
            .persist(WorkOrderItem {
                status: WorkOrderItemStatus::Resolved,
                resolved_at: Some(chrono::Utc::now().naive_utc()),
                resolution_description: Some(format!(
                    "Inspected at {inspection_km} km; {granted_km} km granted, new limit {limit} km; no part replaced. {description}"
                )),
                ..item
            })
            .await
            .map_err(database_error)?;

        let order = WorkOrderEntityMapper::from_model(order);
        if !matches!(order.status, WorkOrderStatus::Concluded | WorkOrderStatus::Cancelled) {
            self.work_orders
                .persist(WorkOrder { status: WorkOrderStatus::PartiallyResolved, ..order })
                .await
                .map_err(database_error)?;
        }
        Ok(PreventivePlanExtensionEntityMapper::from_active_model(saved))
    }

    /// `TRM-306`: opens a work order for a plan that is overdue or needs
    /// attention, unless one is already open for it -- then that one is
    /// returned (`false` = not newly created), so calling this twice never
    /// duplicates. Persisted through the gateway directly, not
    /// `WorkOrderUseCase::create` (which forces `Manual` origin and would
    /// write a second odometer reading -- the same reason
    /// `ChecklistRunUseCase` bypasses it). The odometer is the vehicle's
    /// current one, `0.0` if none was ever recorded.
    pub async fn generate_work_order(&self, uuid: String) -> Result<(WorkOrder, bool), BusinessError> {
        let (plan, status) = self.find_by_uuid(uuid).await?;
        if status == PreventiveStatus::Ok {
            return Err(BusinessError::new(PLAN_NOT_DUE.to_string()));
        }
        let plan_id = plan.id.unwrap_or_default();

        let active = self
            .work_orders
            .find_active_by_preventive_plan(plan_id)
            .await
            .map_err(database_error)?;
        if let Some(existing) = active.into_iter().next() {
            return Ok((WorkOrderEntityMapper::from_model(existing), false));
        }

        let odometer_km = self
            .vehicles
            .find_by_id(plan.vehicle_id)
            .await
            .map_err(database_error)?
            .and_then(|v| v.odometer_km)
            .unwrap_or(0.0);
        let work_order = WorkOrder {
            id: None,
            uuid: None,
            tenant_id: plan.tenant_id,
            vehicle_id: plan.vehicle_id,
            opened_at: None,
            odometer_km,
            origin: WorkOrderOrigin::Preventive,
            checklist_run_id: None,
            maintenance_plan_id: None,
            preventive_plan_id: plan.id,
            service_type: None,
            description: format!("Preventive maintenance: {}", plan.plan_name),
            responsible: None,
            status: WorkOrderStatus::Open,
            observation: None,
            external_service: false,
            supplier: None,
            invoice_number: None,
            invoice_value_cents: None,
            invoice_date: None,
            concluded_at: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        };
        let entity = self.work_orders.persist(work_order).await.map_err(database_error)?;
        Ok((WorkOrderEntityMapper::from_active_model(entity), true))
    }

    pub async fn create(&self, plan: PreventivePlan) -> Result<PreventivePlan, BusinessError> {
        let plan = self.validated(plan).await?;
        let entity = self.gateway.persist(plan).await.map_err(database_error)?;
        Ok(PreventivePlanEntityMapper::from_active_model(entity))
    }

    async fn validated(&self, plan: PreventivePlan) -> Result<PreventivePlan, BusinessError> {
        let plan_name = plan.plan_name.trim().to_string();
        if plan_name.is_empty() {
            return Err(BusinessError::new(PLAN_NAME_REQUIRED.to_string()));
        }

        check_intervals(&plan)?;

        let vehicle = self
            .vehicles
            .find_by_id(plan.vehicle_id)
            .await
            .map_err(database_error)?;
        if !vehicle.is_some_and(|v| v.tenant_id == plan.tenant_id) {
            return Err(BusinessError::new(VEHICLE_NOT_FOUND.to_string()));
        }

        let existing = self
            .gateway
            .find_by_vehicle_and_name(plan.vehicle_id, &plan_name)
            .await
            .map_err(database_error)?;
        if existing.is_some() {
            return Err(BusinessError::new(DUPLICATE_PLAN.to_string()));
        }

        Ok(PreventivePlan { plan_name, ..plan })
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<(PreventivePlan, PreventiveStatus), BusinessError> {
        let entity = self.gateway.find_by_uuid(uuid).await.map_err(database_error)?;
        let plan = match entity {
            Some(model) => PreventivePlanEntityMapper::from_model(model),
            None => return Err(BusinessError::new(PREVENTIVE_PLAN_NOT_FOUND.to_string())),
        };
        let status = self.status_of(&plan).await?;
        Ok((plan, status))
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<(PreventivePlan, PreventiveStatus)>, u64), BusinessError> {
        let (rows, total) = self.gateway.find_page(page, page_size).await.map_err(database_error)?;
        let mut plans = Vec::with_capacity(rows.len());
        for plan in PreventivePlanEntityMapper::from_models(rows) {
            let status = self.status_of(&plan).await?;
            plans.push((plan, status));
        }
        Ok((plans, total))
    }

    /// `TRM-302`/`303`: derived fresh from the vehicle's current odometer and
    /// today's date every time -- never stored, so it can never drift from
    /// them (the same discipline `TRM-602` already established for stock).
    /// Missing data on one axis (no last service recorded yet) contributes no
    /// warning on that axis rather than a false "overdue."
    async fn status_of(&self, plan: &PreventivePlan) -> Result<PreventiveStatus, BusinessError> {
        let vehicle = self
            .vehicles
            .find_by_id(plan.vehicle_id)
            .await
            .map_err(database_error)?;
        let current_km = vehicle.and_then(|v| v.odometer_km);
        Ok(compute_status(plan, current_km, chrono::Utc::now().date_naive()))
    }
}

fn compute_status(plan: &PreventivePlan, current_km: Option<f64>, today: chrono::NaiveDate) -> PreventiveStatus {
    let mut status = PreventiveStatus::Ok;

    if matches!(plan.control_type, PreventiveControlType::Kilometers | PreventiveControlType::Both) {
        // `TRM-316`: an active extension's limit replaces last + interval.
        let next = plan.extension_limit_km.or(match (plan.last_service_km, plan.interval_km) {
            (Some(last), Some(interval)) => Some(last + interval),
            _ => None,
        });
        if let (Some(next), Some(current)) = (next, current_km) {
            status = worse(status, if current >= next {
                PreventiveStatus::Overdue
            } else if current >= next - ATTENTION_KM_MARGIN {
                PreventiveStatus::Attention
            } else {
                PreventiveStatus::Ok
            });
        }
    }

    if matches!(plan.control_type, PreventiveControlType::Days | PreventiveControlType::Both)
        && let (Some(last), Some(interval)) = (plan.last_service_date, plan.interval_days)
    {
        let next = last + Duration::days(interval as i64);
        status = worse(status, if today >= next {
            PreventiveStatus::Overdue
        } else if today >= next - Duration::days(ATTENTION_DAYS_MARGIN) {
            PreventiveStatus::Attention
        } else {
            PreventiveStatus::Ok
        });
    }

    status
}

/// `TRM-301`: the control type needs its matching interval(s).
fn check_intervals(plan: &PreventivePlan) -> Result<(), BusinessError> {
    let has_km = plan.interval_km.is_some_and(|v| v > 0.0);
    let has_days = plan.interval_days.is_some_and(|v| v > 0);
    let satisfied = match plan.control_type {
        PreventiveControlType::Kilometers => has_km,
        PreventiveControlType::Days => has_days,
        PreventiveControlType::Both => has_km && has_days,
    };
    if satisfied { Ok(()) } else { Err(BusinessError::new(INTERVAL_REQUIRED.to_string())) }
}

/// `TRM-301`: a plan controlled by both counts as overdue if *either* axis
/// says so -- the worse of the two always wins.
fn worse(a: PreventiveStatus, b: PreventiveStatus) -> PreventiveStatus {
    fn rank(s: &PreventiveStatus) -> u8 {
        match s {
            PreventiveStatus::Ok => 0,
            PreventiveStatus::Attention => 1,
            PreventiveStatus::Overdue => 2,
        }
    }
    if rank(&b) > rank(&a) { b } else { a }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[PreventivePlanUseCase] {}", msg);
    BusinessError::new(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn plan(control_type: PreventiveControlType) -> PreventivePlan {
        PreventivePlan {
            id: None,
            uuid: None,
            tenant_id: Some(1),
            vehicle_id: 1,
            plan_name: "Oil".into(),
            control_type,
            interval_km: Some(10_000.0),
            interval_days: Some(90),
            last_service_km: Some(50_000.0),
            last_service_date: NaiveDate::from_ymd_opt(2026, 1, 1),
            extension_limit_km: None,
            last_work_order_id: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }
    }

    fn day(m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, d).unwrap()
    }

    #[test]
    fn kilometre_plan_goes_ok_then_attention_then_overdue() {
        let p = plan(PreventiveControlType::Kilometers);
        assert_eq!(compute_status(&p, Some(55_000.0), day(9, 1)), PreventiveStatus::Ok);
        assert_eq!(compute_status(&p, Some(59_000.0), day(9, 1)), PreventiveStatus::Attention);
        assert_eq!(compute_status(&p, Some(60_000.0), day(9, 1)), PreventiveStatus::Overdue);
    }

    #[test]
    fn date_plan_goes_ok_then_attention_then_overdue() {
        let p = plan(PreventiveControlType::Days); // next = 2026-04-01
        assert_eq!(compute_status(&p, None, day(2, 1)), PreventiveStatus::Ok);
        assert_eq!(compute_status(&p, None, day(3, 20)), PreventiveStatus::Attention);
        assert_eq!(compute_status(&p, None, day(4, 1)), PreventiveStatus::Overdue);
    }

    #[test]
    fn a_plan_controlled_by_both_takes_the_worse_axis() {
        let p = plan(PreventiveControlType::Both);
        assert_eq!(compute_status(&p, Some(55_000.0), day(4, 1)), PreventiveStatus::Overdue);
        assert_eq!(compute_status(&p, Some(60_000.0), day(2, 1)), PreventiveStatus::Overdue);
    }

    #[test]
    fn missing_data_is_never_a_false_overdue() {
        let mut p = plan(PreventiveControlType::Both);
        p.last_service_km = None;
        p.last_service_date = None;
        assert_eq!(compute_status(&p, Some(999_999.0), day(12, 1)), PreventiveStatus::Ok);
        let p = plan(PreventiveControlType::Kilometers);
        assert_eq!(compute_status(&p, None, day(12, 1)), PreventiveStatus::Ok);
    }

    #[test]
    fn an_active_extension_limit_replaces_last_plus_interval() {
        let mut p = plan(PreventiveControlType::Kilometers); // due at 60,000
        assert_eq!(compute_status(&p, Some(60_500.0), day(9, 1)), PreventiveStatus::Overdue);
        p.extension_limit_km = Some(65_000.0);
        assert_eq!(compute_status(&p, Some(60_500.0), day(9, 1)), PreventiveStatus::Ok);
        assert_eq!(compute_status(&p, Some(64_500.0), day(9, 1)), PreventiveStatus::Attention);
        assert_eq!(compute_status(&p, Some(65_000.0), day(9, 1)), PreventiveStatus::Overdue);
    }
}
