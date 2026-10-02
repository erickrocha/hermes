use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `HRMS-954`: a tenant's rule thresholds -- the legacy `[TC]` values are only
/// defaults. On `GET`, `uuid` is absent while the tenant is still on them.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TenantRuleSettingJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<i64>,
    /// Default 4000.0. `TRM-564`: a fuelling segment longer than this is discarded
    pub avg_max_segment_km: f64,
    /// Default 0.8. `TRM-564`: a segment below this km/l is discarded
    pub avg_min_km_per_liter: f64,
    /// Default 15.0. `TRM-564`: a segment above this km/l is discarded
    pub avg_max_km_per_liter: f64,
    /// Default 30. `TRM-565`: segments closed within this many days are averaged
    pub avg_recent_window_days: i32,
    /// Default 8. `TRM-565`: with none recent, the last this-many segments
    pub avg_fallback_segments: i32,
    /// Default 4. `TRM-566`: median rejection applies from this many segments
    pub avg_median_min_segments: i32,
    /// Default 0.4. `TRM-566`: a segment this share away from the median is rejected
    pub avg_median_tolerance: f64,
    /// Default 2. `TRM-566`: rejection applies only if this many survive
    pub avg_median_min_survivors: i32,
    /// Default 3. `TRM-568`: the vehicle's own average is used from this many segments
    pub avg_min_segments_for_own_average: i32,
    /// Default 0.5. `TRM-541`: a second report within this many litres is the same fuelling
    pub receipt_duplicate_volume_tolerance_liters: f64,
    /// Default 10. `TRM-541`: ... made within this many minutes
    pub receipt_duplicate_window_minutes: i32,
    /// Default 48. `TRM-545`: a provider posting matches a driver report within this many hours
    pub reconciliation_window_hours: i32,
    /// Default 1.0. `TRM-545`: ... and within the greater of this many litres
    pub reconciliation_volume_tolerance_liters: f64,
    /// Default 0.03. `TRM-545`: ... and this share of the volume
    pub reconciliation_volume_tolerance_ratio: f64,
    /// Default 1000.0. `TRM-303`: a plan this many km from its next service needs attention
    pub preventive_attention_km_margin: f64,
    /// Default 15. `TRM-303`: ... or this many days
    pub preventive_attention_days_margin: i32,
    /// Default 50000.0. `TRM-313`: the most one extension may grant
    pub preventive_max_extension_km: f64,
    /// Default 0.3. `TRM-570`: a peer's tank capacity within this share of the vehicle's
    pub avg_peer_capacity_tolerance: f64,
    /// Default 2. `TRM-570`: at least this many such peers are needed
    pub avg_peer_min_count: i32,
    /// Default 36. `TRM-450`: a performed service expires this many hours after the vehicle's last departure
    pub garage_validity_hours: i32,
    /// Default 60. `TRM-466/782`: an absence at least this long counts as a trip
    pub garage_min_trip_absence_minutes: i32,
    /// Default 50. `TRM-462`: below this tank level the fuelling service is pending on its own
    pub garage_fuel_pending_below_percent: f64,
    /// Default 95. `TRM-468`: at or above this level a trip need not redo fuelling
    pub garage_trip_fuel_exempt_percent: f64,
    /// Default 40. `TRM-474/1401`: below this tank level the fill-the-tank alert is raised regardless of any departure, and a vehicle is called to base whatever its fuelling service says
    pub garage_alert_low_percent: f64,
    /// Default 95. `TRM-474`: a vehicle going to travel is alerted below this tank level
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
}
