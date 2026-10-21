use business::domain::tenant_dashboard::TenantDashboard;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TenantDashboardJson {
    pub active_vehicles: u64,
    pub vehicles_in_maintenance: u64,
    pub open_work_orders: u64,
    pub work_orders_awaiting_parts: u64,
    pub yard_queue: u64,
    pub unread_maintenance_notices: u64,
    pub unjustified_non_executions: u64,
    pub enabled_users: u64,
    /// Absent unless the caller is the tenant owner.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overdue_charges: Option<u64>,
}

impl From<TenantDashboard> for TenantDashboardJson {
    fn from(snapshot: TenantDashboard) -> Self {
        Self {
            active_vehicles: snapshot.active_vehicles,
            vehicles_in_maintenance: snapshot.vehicles_in_maintenance,
            open_work_orders: snapshot.open_work_orders,
            work_orders_awaiting_parts: snapshot.work_orders_awaiting_parts,
            yard_queue: snapshot.yard_queue,
            unread_maintenance_notices: snapshot.unread_maintenance_notices,
            unjustified_non_executions: snapshot.unjustified_non_executions,
            enabled_users: snapshot.enabled_users,
            overdue_charges: snapshot.overdue_charges,
        }
    }
}
