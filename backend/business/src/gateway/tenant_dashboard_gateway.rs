use crate::commons::gateway::tenant_select;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::{VehicleStatus, WorkOrderStatus};
use crate::domain::tenant_dashboard::PendingChargeWindow;
use entity::billing_charge_entity::Entity as BillingCharge;
use entity::garage_attendance_entity::Entity as GarageAttendance;
use entity::maintenance_notification_entity::Entity as MaintenanceNotification;
use entity::non_execution_occurrence_entity::Entity as NonExecutionOccurrence;
use entity::user_entity::Entity as UserQuery;
use entity::vehicle_entity::Entity as VehicleQuery;
use entity::work_order_entity::Entity as WorkOrderQuery;
use entity::{
    billing_charge_entity, garage_attendance_entity, maintenance_notification_entity,
    non_execution_occurrence_entity, user_entity, vehicle_entity, work_order_entity,
};
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, FromQueryResult, PaginatorTrait,
    QueryFilter, Select,
};

const OPEN_WORK_ORDERS: [&str; 3] = ["Open", "PartiallyResolved", "AwaitingParts"];

/// Operational totals for the caller's tenant. Every read goes through
/// `tenant_select`, so an unbound scope cannot widen these into a platform count.
#[async_trait]
pub trait TenantDashboardStore: Send + Sync {
    async fn active_vehicles(&self) -> Result<u64, BusinessError>;
    async fn vehicles_in_maintenance(&self) -> Result<u64, BusinessError>;
    async fn open_work_orders(&self) -> Result<u64, BusinessError>;
    async fn work_orders_awaiting_parts(&self) -> Result<u64, BusinessError>;
    async fn yard_queue(&self) -> Result<u64, BusinessError>;
    async fn unread_maintenance_notices(&self) -> Result<u64, BusinessError>;
    async fn unjustified_non_executions(&self) -> Result<u64, BusinessError>;
    async fn enabled_users(&self) -> Result<u64, BusinessError>;
    async fn pending_charges(&self) -> Result<Vec<PendingChargeWindow>, BusinessError>;
}

pub struct TenantDashboardGateway {
    db: DatabaseConnection,
}

impl TenantDashboardGateway {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TenantDashboardStore for TenantDashboardGateway {
    async fn active_vehicles(&self) -> Result<u64, BusinessError> {
        count(
            &self.db,
            tenant_select(VehicleQuery::find(), vehicle_entity::Column::TenantId)
                .filter(vehicle_entity::Column::Status.eq(VehicleStatus::Active.to_string())),
        )
        .await
    }

    async fn vehicles_in_maintenance(&self) -> Result<u64, BusinessError> {
        count(
            &self.db,
            tenant_select(VehicleQuery::find(), vehicle_entity::Column::TenantId)
                .filter(vehicle_entity::Column::Status.eq(VehicleStatus::Maintenance.to_string())),
        )
        .await
    }

    async fn open_work_orders(&self) -> Result<u64, BusinessError> {
        count(
            &self.db,
            tenant_select(WorkOrderQuery::find(), work_order_entity::Column::TenantId).filter(
                work_order_entity::Column::Status.is_in(
                    OPEN_WORK_ORDERS
                        .into_iter()
                        .map(str::to_string)
                        .collect::<Vec<_>>(),
                ),
            ),
        )
        .await
    }

    async fn work_orders_awaiting_parts(&self) -> Result<u64, BusinessError> {
        count(
            &self.db,
            tenant_select(WorkOrderQuery::find(), work_order_entity::Column::TenantId).filter(
                work_order_entity::Column::Status.eq(WorkOrderStatus::AwaitingParts.to_string()),
            ),
        )
        .await
    }

    async fn yard_queue(&self) -> Result<u64, BusinessError> {
        count(
            &self.db,
            tenant_select(
                GarageAttendance::find(),
                garage_attendance_entity::Column::TenantId,
            )
            .filter(garage_attendance_entity::Column::ActiveMarker.is_not_null()),
        )
        .await
    }

    async fn unread_maintenance_notices(&self) -> Result<u64, BusinessError> {
        count(
            &self.db,
            tenant_select(
                MaintenanceNotification::find(),
                maintenance_notification_entity::Column::TenantId,
            )
            .filter(maintenance_notification_entity::Column::ReadAt.is_null()),
        )
        .await
    }

    async fn unjustified_non_executions(&self) -> Result<u64, BusinessError> {
        count(
            &self.db,
            tenant_select(
                NonExecutionOccurrence::find(),
                non_execution_occurrence_entity::Column::TenantId,
            )
            .filter(non_execution_occurrence_entity::Column::Justification.is_null()),
        )
        .await
    }

    async fn enabled_users(&self) -> Result<u64, BusinessError> {
        count(
            &self.db,
            tenant_select(UserQuery::find(), user_entity::Column::TenantId)
                .filter(user_entity::Column::Enabled.eq(true)),
        )
        .await
    }

    async fn pending_charges(&self) -> Result<Vec<PendingChargeWindow>, BusinessError> {
        tenant_select(
            BillingCharge::find(),
            billing_charge_entity::Column::TenantId,
        )
        .filter(billing_charge_entity::Column::Status.eq("Pending"))
        .all(&self.db)
        .await
        .map(|models| {
            models
                .into_iter()
                .map(|model| PendingChargeWindow {
                    period_end: model.period_end.naive_utc(),
                    grace_days: model.grace_days,
                })
                .collect()
        })
        .map_err(database_error)
    }
}

async fn count<C, E>(db: &C, query: Select<E>) -> Result<u64, BusinessError>
where
    C: ConnectionTrait,
    E: EntityTrait,
    E::Model: FromQueryResult + Sized + Send + Sync,
{
    query.count(db).await.map_err(database_error)
}

fn database_error(error: sea_orm::DbErr) -> BusinessError {
    let message = format!("Tenant dashboard database error: {error}");
    log::error!("[TenantDashboardGateway] {message}");
    BusinessError::new(message)
}
