use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::{PreventiveControlType, PreventiveStatus};
use crate::domain::preventive_plan::{PreventivePlan, PreventivePlanEntityMapper};
use crate::gateway::preventive_plan_gateway::PreventivePlanGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use chrono::Duration;
use sea_orm::DbErr;

pub const PLAN_NAME_REQUIRED: &str = "A preventive plan needs a name";
pub const INTERVAL_REQUIRED: &str = "The control type needs its matching interval (kilometres, days, or both)";
pub const VEHICLE_NOT_FOUND: &str = "Vehicle not found";
pub const DUPLICATE_PLAN: &str = "This vehicle already has a preventive plan with this name";
pub const PREVENTIVE_PLAN_NOT_FOUND: &str = "Preventive plan not found";

/// `TRM-303`'s own `TC`: within 1,000 km or 15 days of the next service is
/// "requiring attention," not yet overdue.
pub const ATTENTION_KM_MARGIN: f64 = 1_000.0;
pub const ATTENTION_DAYS_MARGIN: i64 = 15;

pub struct PreventivePlanUseCase {
    gateway: PreventivePlanGateway,
    vehicles: VehicleGateway,
}

impl PreventivePlanUseCase {
    pub fn new(gateway: PreventivePlanGateway, vehicles: VehicleGateway) -> Self {
        Self { gateway, vehicles }
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

        let has_km = plan.interval_km.is_some_and(|v| v > 0.0);
        let has_days = plan.interval_days.is_some_and(|v| v > 0);
        let satisfied = match plan.control_type {
            PreventiveControlType::Kilometers => has_km,
            PreventiveControlType::Days => has_days,
            PreventiveControlType::Both => has_km && has_days,
        };
        if !satisfied {
            return Err(BusinessError::new(INTERVAL_REQUIRED.to_string()));
        }

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
        let today = chrono::Utc::now().date_naive();
        let mut status = PreventiveStatus::Ok;

        if matches!(plan.control_type, PreventiveControlType::Kilometers | PreventiveControlType::Both)
            && let (Some(last), Some(interval), Some(current)) = (plan.last_service_km, plan.interval_km, current_km)
        {
            let next = last + interval;
            status = worse(status, if current >= next {
                PreventiveStatus::Overdue
            } else if current >= next - ATTENTION_KM_MARGIN {
                PreventiveStatus::Attention
            } else {
                PreventiveStatus::Ok
            });
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

        Ok(status)
    }
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
