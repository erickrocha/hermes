use chrono::NaiveDateTime;

/// Counts the caller's own tenant can act on. Billing is optional: only the
/// tenant owner is shown charges, and an unbound caller never gets this far.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TenantDashboard {
    pub active_vehicles: u64,
    pub vehicles_in_maintenance: u64,
    pub open_work_orders: u64,
    pub work_orders_awaiting_parts: u64,
    pub yard_queue: u64,
    pub unread_maintenance_notices: u64,
    pub unjustified_non_executions: u64,
    pub enabled_users: u64,
    pub overdue_charges: Option<u64>,
}

/// A pending charge's window, so "overdue" stays the same rule the billing
/// screen already uses (`period_end + grace_days`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingChargeWindow {
    pub period_end: NaiveDateTime,
    pub grace_days: i32,
}
