use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::NaiveDateTime;
use entity::tenant_rule_setting_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};

/// `HRMS-954`: a tenant's rule thresholds, in force for that tenant alone.
/// The legacy `[TC]` values are [`Default`]; a tenant with no row gets them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuleSettings {
    /// `TRM-564`: a fuelling segment longer than this is discarded
    pub avg_max_segment_km: f64,
    /// `TRM-564`: a segment below this km/l is discarded
    pub avg_min_km_per_liter: f64,
    /// `TRM-564`: a segment above this km/l is discarded
    pub avg_max_km_per_liter: f64,
    /// `TRM-565`: segments closed within this many days are averaged
    pub avg_recent_window_days: i32,
    /// `TRM-565`: with none recent, the last this-many segments
    pub avg_fallback_segments: i32,
    /// `TRM-566`: median rejection applies from this many segments
    pub avg_median_min_segments: i32,
    /// `TRM-566`: a segment this share away from the median is rejected
    pub avg_median_tolerance: f64,
    /// `TRM-566`: rejection applies only if this many survive
    pub avg_median_min_survivors: i32,
    /// `TRM-568`: the vehicle's own average is used from this many segments
    pub avg_min_segments_for_own_average: i32,
    /// `TRM-541`: a second report within this many litres is the same fuelling
    pub receipt_duplicate_volume_tolerance_liters: f64,
    /// `TRM-541`: ... made within this many minutes
    pub receipt_duplicate_window_minutes: i32,
    /// `TRM-545`: a provider posting matches a driver report within this many hours
    pub reconciliation_window_hours: i32,
    /// `TRM-545`: ... and within the greater of this many litres
    pub reconciliation_volume_tolerance_liters: f64,
    /// `TRM-545`: ... and this share of the volume
    pub reconciliation_volume_tolerance_ratio: f64,
    /// `TRM-303`: a plan this many km from its next service needs attention
    pub preventive_attention_km_margin: f64,
    /// `TRM-303`: ... or this many days
    pub preventive_attention_days_margin: i32,
    /// `TRM-313`: the most one extension may grant
    pub preventive_max_extension_km: f64,
    /// `TRM-570`: a peer's tank capacity within this share of the vehicle's
    pub avg_peer_capacity_tolerance: f64,
    /// `TRM-570`: at least this many such peers are needed
    pub avg_peer_min_count: i32,
    /// `TRM-450`: a performed service expires this many hours after the vehicle's last departure
    pub garage_validity_hours: i32,
    /// `TRM-466/782`: an absence at least this long counts as a trip
    pub garage_min_trip_absence_minutes: i32,
    /// `TRM-462`: below this tank level the fuelling service is pending on its own
    pub garage_fuel_pending_below_percent: f64,
    /// `TRM-468`: at or above this level a trip need not redo fuelling
    pub garage_trip_fuel_exempt_percent: f64,
    /// `TRM-474/1401`: below this tank level the fill-the-tank alert is raised regardless of any departure, and a vehicle is called to base whatever its fuelling service says
    pub garage_alert_low_percent: f64,
    /// `TRM-474`: a vehicle going to travel is alerted below this tank level
    pub garage_alert_trip_percent: f64,
}

impl Default for RuleSettings {
    fn default() -> Self {
        Self {
            avg_max_segment_km: 4_000.0,
            avg_min_km_per_liter: 0.8,
            avg_max_km_per_liter: 15.0,
            avg_recent_window_days: 30,
            avg_fallback_segments: 8,
            avg_median_min_segments: 4,
            avg_median_tolerance: 0.4,
            avg_median_min_survivors: 2,
            avg_min_segments_for_own_average: 3,
            receipt_duplicate_volume_tolerance_liters: 0.5,
            receipt_duplicate_window_minutes: 10,
            reconciliation_window_hours: 48,
            reconciliation_volume_tolerance_liters: 1.0,
            reconciliation_volume_tolerance_ratio: 0.03,
            preventive_attention_km_margin: 1_000.0,
            preventive_attention_days_margin: 15,
            preventive_max_extension_km: 50_000.0,
            avg_peer_capacity_tolerance: 0.3,
            avg_peer_min_count: 2,
            garage_validity_hours: 36,
            garage_min_trip_absence_minutes: 60,
            garage_fuel_pending_below_percent: 50.0,
            garage_trip_fuel_exempt_percent: 95.0,
            garage_alert_low_percent: 40.0,
            garage_alert_trip_percent: 95.0,
        }
    }
}

impl From<&Model> for RuleSettings {
    fn from(m: &Model) -> Self {
        Self {
            avg_max_segment_km: m.avg_max_segment_km,
            avg_min_km_per_liter: m.avg_min_km_per_liter,
            avg_max_km_per_liter: m.avg_max_km_per_liter,
            avg_recent_window_days: m.avg_recent_window_days,
            avg_fallback_segments: m.avg_fallback_segments,
            avg_median_min_segments: m.avg_median_min_segments,
            avg_median_tolerance: m.avg_median_tolerance,
            avg_median_min_survivors: m.avg_median_min_survivors,
            avg_min_segments_for_own_average: m.avg_min_segments_for_own_average,
            receipt_duplicate_volume_tolerance_liters: m.receipt_duplicate_volume_tolerance_liters,
            receipt_duplicate_window_minutes: m.receipt_duplicate_window_minutes,
            reconciliation_window_hours: m.reconciliation_window_hours,
            reconciliation_volume_tolerance_liters: m.reconciliation_volume_tolerance_liters,
            reconciliation_volume_tolerance_ratio: m.reconciliation_volume_tolerance_ratio,
            preventive_attention_km_margin: m.preventive_attention_km_margin,
            preventive_attention_days_margin: m.preventive_attention_days_margin,
            preventive_max_extension_km: m.preventive_max_extension_km,
            avg_peer_capacity_tolerance: m.avg_peer_capacity_tolerance,
            avg_peer_min_count: m.avg_peer_min_count,
            garage_validity_hours: m.garage_validity_hours,
            garage_min_trip_absence_minutes: m.garage_min_trip_absence_minutes,
            garage_fuel_pending_below_percent: m.garage_fuel_pending_below_percent,
            garage_trip_fuel_exempt_percent: m.garage_trip_fuel_exempt_percent,
            garage_alert_low_percent: m.garage_alert_low_percent,
            garage_alert_trip_percent: m.garage_alert_trip_percent,
        }
    }
}

/// One tenant's stored row.
#[derive(Debug, Clone, PartialEq)]
pub struct TenantRuleSetting {
    pub id: Option<i64>,
    pub uuid: Option<String>,
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
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

impl TenantRuleSetting {
    pub fn new(tenant_id: Option<i64>, s: RuleSettings) -> Self {
        Self {
            id: None,
            uuid: None,
            tenant_id,
            avg_max_segment_km: s.avg_max_segment_km,
            avg_min_km_per_liter: s.avg_min_km_per_liter,
            avg_max_km_per_liter: s.avg_max_km_per_liter,
            avg_recent_window_days: s.avg_recent_window_days,
            avg_fallback_segments: s.avg_fallback_segments,
            avg_median_min_segments: s.avg_median_min_segments,
            avg_median_tolerance: s.avg_median_tolerance,
            avg_median_min_survivors: s.avg_median_min_survivors,
            avg_min_segments_for_own_average: s.avg_min_segments_for_own_average,
            receipt_duplicate_volume_tolerance_liters: s.receipt_duplicate_volume_tolerance_liters,
            receipt_duplicate_window_minutes: s.receipt_duplicate_window_minutes,
            reconciliation_window_hours: s.reconciliation_window_hours,
            reconciliation_volume_tolerance_liters: s.reconciliation_volume_tolerance_liters,
            reconciliation_volume_tolerance_ratio: s.reconciliation_volume_tolerance_ratio,
            preventive_attention_km_margin: s.preventive_attention_km_margin,
            preventive_attention_days_margin: s.preventive_attention_days_margin,
            preventive_max_extension_km: s.preventive_max_extension_km,
            avg_peer_capacity_tolerance: s.avg_peer_capacity_tolerance,
            avg_peer_min_count: s.avg_peer_min_count,
            garage_validity_hours: s.garage_validity_hours,
            garage_min_trip_absence_minutes: s.garage_min_trip_absence_minutes,
            garage_fuel_pending_below_percent: s.garage_fuel_pending_below_percent,
            garage_trip_fuel_exempt_percent: s.garage_trip_fuel_exempt_percent,
            garage_alert_low_percent: s.garage_alert_low_percent,
            garage_alert_trip_percent: s.garage_alert_trip_percent,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }
    }

    pub fn settings(&self) -> RuleSettings {
        RuleSettings {
            avg_max_segment_km: self.avg_max_segment_km,
            avg_min_km_per_liter: self.avg_min_km_per_liter,
            avg_max_km_per_liter: self.avg_max_km_per_liter,
            avg_recent_window_days: self.avg_recent_window_days,
            avg_fallback_segments: self.avg_fallback_segments,
            avg_median_min_segments: self.avg_median_min_segments,
            avg_median_tolerance: self.avg_median_tolerance,
            avg_median_min_survivors: self.avg_median_min_survivors,
            avg_min_segments_for_own_average: self.avg_min_segments_for_own_average,
            receipt_duplicate_volume_tolerance_liters: self.receipt_duplicate_volume_tolerance_liters,
            receipt_duplicate_window_minutes: self.receipt_duplicate_window_minutes,
            reconciliation_window_hours: self.reconciliation_window_hours,
            reconciliation_volume_tolerance_liters: self.reconciliation_volume_tolerance_liters,
            reconciliation_volume_tolerance_ratio: self.reconciliation_volume_tolerance_ratio,
            preventive_attention_km_margin: self.preventive_attention_km_margin,
            preventive_attention_days_margin: self.preventive_attention_days_margin,
            preventive_max_extension_km: self.preventive_max_extension_km,
            avg_peer_capacity_tolerance: self.avg_peer_capacity_tolerance,
            avg_peer_min_count: self.avg_peer_min_count,
            garage_validity_hours: self.garage_validity_hours,
            garage_min_trip_absence_minutes: self.garage_min_trip_absence_minutes,
            garage_fuel_pending_below_percent: self.garage_fuel_pending_below_percent,
            garage_trip_fuel_exempt_percent: self.garage_trip_fuel_exempt_percent,
            garage_alert_low_percent: self.garage_alert_low_percent,
            garage_alert_trip_percent: self.garage_alert_trip_percent,
        }
    }
}

pub struct TenantRuleSettingEntityMapper {}

impl EntityMapper<TenantRuleSetting, Model, ActiveModel> for TenantRuleSettingEntityMapper {
    fn build_active_model(d: TenantRuleSetting) -> ActiveModel {
        ActiveModel {
            id: match d.id {
                Some(id) => Set(id),
                None => NotSet,
            },
            uuid: match d.uuid {
                Some(uuid) => Set(string_to_bytes(&uuid)),
                None => NotSet,
            },
            tenant_id: Set(d.tenant_id),
            avg_max_segment_km: Set(d.avg_max_segment_km),
            avg_min_km_per_liter: Set(d.avg_min_km_per_liter),
            avg_max_km_per_liter: Set(d.avg_max_km_per_liter),
            avg_recent_window_days: Set(d.avg_recent_window_days),
            avg_fallback_segments: Set(d.avg_fallback_segments),
            avg_median_min_segments: Set(d.avg_median_min_segments),
            avg_median_tolerance: Set(d.avg_median_tolerance),
            avg_median_min_survivors: Set(d.avg_median_min_survivors),
            avg_min_segments_for_own_average: Set(d.avg_min_segments_for_own_average),
            receipt_duplicate_volume_tolerance_liters: Set(d.receipt_duplicate_volume_tolerance_liters),
            receipt_duplicate_window_minutes: Set(d.receipt_duplicate_window_minutes),
            reconciliation_window_hours: Set(d.reconciliation_window_hours),
            reconciliation_volume_tolerance_liters: Set(d.reconciliation_volume_tolerance_liters),
            reconciliation_volume_tolerance_ratio: Set(d.reconciliation_volume_tolerance_ratio),
            preventive_attention_km_margin: Set(d.preventive_attention_km_margin),
            preventive_attention_days_margin: Set(d.preventive_attention_days_margin),
            preventive_max_extension_km: Set(d.preventive_max_extension_km),
            avg_peer_capacity_tolerance: Set(d.avg_peer_capacity_tolerance),
            avg_peer_min_count: Set(d.avg_peer_min_count),
            garage_validity_hours: Set(d.garage_validity_hours),
            garage_min_trip_absence_minutes: Set(d.garage_min_trip_absence_minutes),
            garage_fuel_pending_below_percent: Set(d.garage_fuel_pending_below_percent),
            garage_trip_fuel_exempt_percent: Set(d.garage_trip_fuel_exempt_percent),
            garage_alert_low_percent: Set(d.garage_alert_low_percent),
            garage_alert_trip_percent: Set(d.garage_alert_trip_percent),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> TenantRuleSetting {
        TenantRuleSetting {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            avg_max_segment_km: e.avg_max_segment_km,
            avg_min_km_per_liter: e.avg_min_km_per_liter,
            avg_max_km_per_liter: e.avg_max_km_per_liter,
            avg_recent_window_days: e.avg_recent_window_days,
            avg_fallback_segments: e.avg_fallback_segments,
            avg_median_min_segments: e.avg_median_min_segments,
            avg_median_tolerance: e.avg_median_tolerance,
            avg_median_min_survivors: e.avg_median_min_survivors,
            avg_min_segments_for_own_average: e.avg_min_segments_for_own_average,
            receipt_duplicate_volume_tolerance_liters: e.receipt_duplicate_volume_tolerance_liters,
            receipt_duplicate_window_minutes: e.receipt_duplicate_window_minutes,
            reconciliation_window_hours: e.reconciliation_window_hours,
            reconciliation_volume_tolerance_liters: e.reconciliation_volume_tolerance_liters,
            reconciliation_volume_tolerance_ratio: e.reconciliation_volume_tolerance_ratio,
            preventive_attention_km_margin: e.preventive_attention_km_margin,
            preventive_attention_days_margin: e.preventive_attention_days_margin,
            preventive_max_extension_km: e.preventive_max_extension_km,
            avg_peer_capacity_tolerance: e.avg_peer_capacity_tolerance,
            avg_peer_min_count: e.avg_peer_min_count,
            garage_validity_hours: e.garage_validity_hours,
            garage_min_trip_absence_minutes: e.garage_min_trip_absence_minutes,
            garage_fuel_pending_below_percent: e.garage_fuel_pending_below_percent,
            garage_trip_fuel_exempt_percent: e.garage_trip_fuel_exempt_percent,
            garage_alert_low_percent: e.garage_alert_low_percent,
            garage_alert_trip_percent: e.garage_alert_trip_percent,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> TenantRuleSetting {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => TenantRuleSetting {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                avg_max_segment_km: e.avg_max_segment_km.take().unwrap_or_default(),
                avg_min_km_per_liter: e.avg_min_km_per_liter.take().unwrap_or_default(),
                avg_max_km_per_liter: e.avg_max_km_per_liter.take().unwrap_or_default(),
                avg_recent_window_days: e.avg_recent_window_days.take().unwrap_or_default(),
                avg_fallback_segments: e.avg_fallback_segments.take().unwrap_or_default(),
                avg_median_min_segments: e.avg_median_min_segments.take().unwrap_or_default(),
                avg_median_tolerance: e.avg_median_tolerance.take().unwrap_or_default(),
                avg_median_min_survivors: e.avg_median_min_survivors.take().unwrap_or_default(),
                avg_min_segments_for_own_average: e.avg_min_segments_for_own_average.take().unwrap_or_default(),
                receipt_duplicate_volume_tolerance_liters: e.receipt_duplicate_volume_tolerance_liters.take().unwrap_or_default(),
                receipt_duplicate_window_minutes: e.receipt_duplicate_window_minutes.take().unwrap_or_default(),
                reconciliation_window_hours: e.reconciliation_window_hours.take().unwrap_or_default(),
                reconciliation_volume_tolerance_liters: e.reconciliation_volume_tolerance_liters.take().unwrap_or_default(),
                reconciliation_volume_tolerance_ratio: e.reconciliation_volume_tolerance_ratio.take().unwrap_or_default(),
                preventive_attention_km_margin: e.preventive_attention_km_margin.take().unwrap_or_default(),
                preventive_attention_days_margin: e.preventive_attention_days_margin.take().unwrap_or_default(),
                preventive_max_extension_km: e.preventive_max_extension_km.take().unwrap_or_default(),
                avg_peer_capacity_tolerance: e.avg_peer_capacity_tolerance.take().unwrap_or_default(),
                avg_peer_min_count: e.avg_peer_min_count.take().unwrap_or_default(),
                garage_validity_hours: e.garage_validity_hours.take().unwrap_or_default(),
                garage_min_trip_absence_minutes: e.garage_min_trip_absence_minutes.take().unwrap_or_default(),
                garage_fuel_pending_below_percent: e.garage_fuel_pending_below_percent.take().unwrap_or_default(),
                garage_trip_fuel_exempt_percent: e.garage_trip_fuel_exempt_percent.take().unwrap_or_default(),
                garage_alert_low_percent: e.garage_alert_low_percent.take().unwrap_or_default(),
                garage_alert_trip_percent: e.garage_alert_trip_percent.take().unwrap_or_default(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
