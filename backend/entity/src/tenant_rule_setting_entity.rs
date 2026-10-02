use sea_orm::entity::prelude::*;

/// `HRMS-954`: a tenant's rule thresholds (the legacy `[TC]` values are the
/// defaults when it has no row).
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "tenant_rule_setting")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub avg_max_segment_km: f64,
    pub avg_min_km_per_liter: f64,
    pub avg_max_km_per_liter: f64,
    pub avg_recent_window_days: i32,
    pub avg_fallback_segments: i32,
    pub avg_median_min_segments: i32,
    pub avg_median_tolerance: f64,
    pub avg_median_min_survivors: i32,
    pub avg_min_segments_for_own_average: i32,
    pub receipt_duplicate_volume_tolerance_liters: f64,
    pub receipt_duplicate_window_minutes: i32,
    pub reconciliation_window_hours: i32,
    pub reconciliation_volume_tolerance_liters: f64,
    pub reconciliation_volume_tolerance_ratio: f64,
    pub preventive_attention_km_margin: f64,
    pub preventive_attention_days_margin: i32,
    pub preventive_max_extension_km: f64,
    pub avg_peer_capacity_tolerance: f64,
    pub avg_peer_min_count: i32,
    pub garage_validity_hours: i32,
    pub garage_min_trip_absence_minutes: i32,
    pub garage_fuel_pending_below_percent: f64,
    pub garage_trip_fuel_exempt_percent: f64,
    pub garage_alert_low_percent: f64,
    pub garage_alert_trip_percent: f64,
    /// `TRM-498`: an extra trip animates on the monitor this many minutes before departing
    pub garage_monitor_urgent_minutes: i32,
    /// `TRM-499`: a trip alerts on the monitor this many minutes before departing
    pub garage_monitor_trip_window_minutes: i32,
    /// `TRM-499`: a recurring line alerts on the monitor this many minutes before departing
    pub garage_monitor_line_window_minutes: i32,
    /// `TRM-495`: the monitor shows this many vehicle cards
    pub garage_monitor_card_limit: i32,
    /// `TRM-496`: the monitor shows this many triage rows in the services matrix
    pub garage_monitor_matrix_rows: i32,
    /// `TRM-496`: ... and this many service columns
    pub garage_monitor_matrix_columns: i32,
    /// `TRM-1523`: a fuelling older than this many hours never marks the fuelling service automatically
    pub garage_fuelling_freshness_hours: i32,
    /// `TRM-415`/`422`: the tenant's operating day is UTC plus this many minutes (`[TC?]` America/Sao_Paulo, a fixed offset; `U-019` still open)
    pub garage_utc_offset_minutes: i32,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
