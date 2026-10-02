use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::maintenance_plan::{MaintenancePlan, MaintenancePlanEntityMapper};
use crate::domain::enums::MaintenancePlanStatus;
use crate::domain::work_order::{WorkOrder, WorkOrderEntityMapper};
use crate::gateway::maintenance_plan_gateway::MaintenancePlanGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use crate::gateway::work_order_gateway::WorkOrderGateway;
use sea_orm::DbErr;

pub const NOT_A_TENANT_VEHICLE: &str = "The vehicle named does not belong to this tenant";
/// `TRM-234`: two overlapping maintenance windows for the same vehicle on
/// the same date are a data-entry error, not a schedule conflict to resolve.
pub const OVERLAPPING_PLAN_EXISTS: &str =
    "This vehicle already has an active maintenance plan for that date";
pub const WORK_ORDER_NOT_FOUND: &str = "Work order not found";
pub const WORK_ORDER_WRONG_VEHICLE: &str =
    "The work order named does not belong to this plan's own vehicle";
/// `TRM-237`: a work order already scheduled into an active plan is not
/// available to a second one; an inactive plan's work orders are.
pub const WORK_ORDER_ALREADY_SCHEDULED: &str =
    "This work order is already scheduled into another active maintenance plan";

pub struct MaintenancePlanUseCase {
    gateway: MaintenancePlanGateway,
    work_orders: WorkOrderGateway,
    vehicles: VehicleGateway,
}

impl MaintenancePlanUseCase {
    pub fn new(
        gateway: MaintenancePlanGateway,
        work_orders: WorkOrderGateway,
        vehicles: VehicleGateway,
    ) -> Self {
        Self {
            gateway,
            work_orders,
            vehicles,
        }
    }

    /// `HRMS-703`/`TRM-230`: scheduling a window and linking the work
    /// orders it covers are one operation. `TRM-234` refuses a second
    /// active plan for the same vehicle and date; `TRM-237` refuses a work
    /// order already scheduled into another *active* plan, but not one
    /// whose plan has since become inactive.
    pub async fn create(
        &self,
        plan: MaintenancePlan,
        work_order_ids: Vec<i64>,
    ) -> Result<(MaintenancePlan, Vec<WorkOrder>), BusinessError> {
        let plan = self.validated(plan).await?;

        let mut work_orders = Vec::with_capacity(work_order_ids.len());
        for id in work_order_ids {
            work_orders.push(self.validated_work_order(id, &plan).await?);
        }

        let entity = self.gateway.persist(plan).await.map_err(database_error)?;
        let saved_plan = MaintenancePlanEntityMapper::from_active_model(entity);

        let mut linked = Vec::with_capacity(work_orders.len());
        for work_order in work_orders {
            let entity = self
                .work_orders
                .persist(WorkOrder {
                    maintenance_plan_id: saved_plan.id,
                    ..work_order
                })
                .await
                .map_err(database_error)?;
            linked.push(WorkOrderEntityMapper::from_active_model(entity));
        }

        Ok((saved_plan, linked))
    }

    async fn validated(&self, plan: MaintenancePlan) -> Result<MaintenancePlan, BusinessError> {
        let vehicle = self
            .vehicles
            .find_by_id(plan.vehicle_id)
            .await
            .map_err(database_error)?;
        if !vehicle.is_some_and(|v| v.tenant_id == plan.tenant_id) {
            return Err(BusinessError::new(NOT_A_TENANT_VEHICLE.to_string()));
        }

        let existing = self
            .gateway
            .find_by_vehicle_and_date(plan.vehicle_id, plan.date)
            .await
            .map_err(database_error)?;
        if existing.iter().any(|p| Self::is_active(&p.status)) {
            return Err(BusinessError::new(OVERLAPPING_PLAN_EXISTS.to_string()));
        }

        Ok(MaintenancePlan {
            status: MaintenancePlanStatus::Scheduled,
            origin: "Manual".to_string(),
            ..plan
        })
    }

    async fn validated_work_order(
        &self,
        work_order_id: i64,
        plan: &MaintenancePlan,
    ) -> Result<WorkOrder, BusinessError> {
        let existing = self
            .work_orders
            .find_by_id(work_order_id)
            .await
            .map_err(database_error)?;
        let Some(existing) = existing else {
            return Err(BusinessError::new(WORK_ORDER_NOT_FOUND.to_string()));
        };
        let work_order = WorkOrderEntityMapper::from_model(existing);
        if work_order.tenant_id != plan.tenant_id || work_order.vehicle_id != plan.vehicle_id {
            return Err(BusinessError::new(WORK_ORDER_WRONG_VEHICLE.to_string()));
        }

        if let Some(current_plan_id) = work_order.maintenance_plan_id {
            let current_plan = self
                .gateway
                .find_by_id(current_plan_id)
                .await
                .map_err(database_error)?;
            if current_plan.is_some_and(|p| Self::is_active(&p.status)) {
                return Err(BusinessError::new(WORK_ORDER_ALREADY_SCHEDULED.to_string()));
            }
        }

        Ok(work_order)
    }

    /// `TRM-237`: `Concluded`, `Cancelled` and `NotExecuted` are inactive;
    /// everything else (today, only `Scheduled`) is active.
    fn is_active(status: &str) -> bool {
        !matches!(status, "Concluded" | "Cancelled" | "NotExecuted")
    }

    pub async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<(MaintenancePlan, Vec<WorkOrder>), BusinessError> {
        let entity = self.gateway.find_by_uuid(uuid).await.map_err(database_error)?;
        let plan = match entity {
            Some(model) => MaintenancePlanEntityMapper::from_model(model),
            None => return Err(BusinessError::new("Maintenance plan not found".to_string())),
        };
        let work_orders = self
            .work_orders
            .find_by_maintenance_plan(plan.id.unwrap_or_default())
            .await
            .map_err(database_error)?;
        Ok((plan, WorkOrderEntityMapper::from_models(work_orders)))
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<MaintenancePlan>, u64), BusinessError> {
        let (rows, total) = self.gateway.find_page(page, page_size).await.map_err(database_error)?;
        Ok((MaintenancePlanEntityMapper::from_models(rows), total))
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[MaintenancePlanUseCase] {}", msg);
    BusinessError::new(msg)
}
