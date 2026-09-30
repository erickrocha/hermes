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
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
