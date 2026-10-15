use crate::domain::business_error::BusinessError;
use crate::domain::tenant_rule_setting::RuleSettings;
use crate::domain::tenant_rule_setting::TenantRuleSetting;
use crate::gateway::tenant_rule_setting_gateway::TenantRuleSettingStore;

pub const SETTING_INVALID: &str = "A rule threshold is outside its sensible range";

/// `HRMS-954`: a tenant's rule thresholds.
pub struct TenantRuleSettingUseCase<G: TenantRuleSettingStore> {
    gateway: G,
}

impl<G: TenantRuleSettingStore> TenantRuleSettingUseCase<G> {
    pub fn new(gateway: G) -> Self {
        Self { gateway }
    }

    /// The tenant's own thresholds, or the legacy defaults when it has set none.
    pub async fn current(
        &self,
        target_tenant_id: Option<i64>,
    ) -> Result<RuleSettings, BusinessError> {
        let setting = self.gateway.get_current(target_tenant_id).await?;
        Ok(setting
            .map(|setting| setting.settings())
            .unwrap_or_default())
    }

    /// Upserts the tenant's single row.
    pub async fn configure(
        &self,
        setting: TenantRuleSetting,
    ) -> Result<TenantRuleSetting, BusinessError> {
        if !is_sensible(&setting.settings()) {
            return Err(BusinessError::new(SETTING_INVALID.to_string()));
        }
        let existing = self.gateway.get_current(setting.tenant_id).await?;
        let setting = match existing {
            Some(existing) => TenantRuleSetting {
                id: existing.id,
                uuid: existing.uuid,
                ..setting
            },
            None => setting,
        };
        self.gateway.save(setting).await
    }
}

/// Every count, window and distance is positive and finite; shares sit in
/// their natural range; the plausible km/l band has a floor below its ceiling.
pub fn is_sensible(s: &RuleSettings) -> bool {
    let positive = |v: f64| v.is_finite() && v > 0.0;
    let share = |v: f64| v.is_finite() && v > 0.0 && v <= 1.0;
    positive(s.avg_max_segment_km)
        && positive(s.avg_min_km_per_liter)
        && positive(s.avg_max_km_per_liter)
        && s.avg_min_km_per_liter < s.avg_max_km_per_liter
        && s.avg_recent_window_days > 0
        && s.avg_fallback_segments > 0
        && s.avg_median_min_segments > 0
        && s.avg_median_tolerance > 0.0
        && s.avg_median_tolerance < 1.0
        && s.avg_median_min_survivors > 0
        && s.avg_min_segments_for_own_average > 0
        && positive(s.receipt_duplicate_volume_tolerance_liters)
        && s.receipt_duplicate_window_minutes > 0
        && s.reconciliation_window_hours > 0
        && positive(s.reconciliation_volume_tolerance_liters)
        && share(s.reconciliation_volume_tolerance_ratio)
        && positive(s.preventive_attention_km_margin)
        && s.preventive_attention_days_margin > 0
        && positive(s.preventive_max_extension_km)
        && s.avg_peer_capacity_tolerance > 0.0
        && s.avg_peer_capacity_tolerance < 1.0
        && s.avg_peer_min_count > 0
        && s.garage_validity_hours > 0
        && s.garage_min_trip_absence_minutes > 0
        && s.garage_fuel_pending_below_percent > 0.0
        && s.garage_trip_fuel_exempt_percent <= 100.0
        && s.garage_fuel_pending_below_percent < s.garage_trip_fuel_exempt_percent
        && s.garage_alert_low_percent > 0.0
        && s.garage_alert_low_percent < s.garage_alert_trip_percent
        && s.garage_alert_trip_percent <= 100.0
        && s.garage_monitor_urgent_minutes > 0
        && s.garage_monitor_trip_window_minutes > 0
        && s.garage_monitor_line_window_minutes > 0
        && s.garage_monitor_card_limit > 0
        && s.garage_monitor_matrix_rows > 0
        && s.garage_monitor_matrix_columns > 0
        && s.garage_fuelling_freshness_hours > 0
        && s.garage_utc_offset_minutes.abs() <= 14 * 60
        && s.garage_base_radius_meters.is_finite()
        && s.garage_base_radius_meters > 0.0
        && s.garage_cycle_fallback_distance_km.is_finite()
        && s.garage_cycle_fallback_distance_km > 0.0
        && s.garage_odometer_max_single_jump_km.is_finite()
        && s.garage_odometer_max_single_jump_km > 0.0
        && match (s.garage_base_latitude, s.garage_base_longitude) {
            (None, None) => true,
            (Some(latitude), Some(longitude)) => {
                latitude.is_finite()
                    && (-90.0..=90.0).contains(&latitude)
                    && longitude.is_finite()
                    && (-180.0..=180.0).contains(&longitude)
            }
            _ => false,
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_legacy_defaults_are_themselves_sensible() {
        assert!(is_sensible(&RuleSettings::default()));
    }

    #[test]
    fn a_value_outside_its_sense_is_refused() {
        let bad = |f: fn(&mut RuleSettings)| {
            let mut s = RuleSettings::default();
            f(&mut s);
            !is_sensible(&s)
        };
        assert!(bad(|s| s.avg_recent_window_days = 0));
        assert!(bad(|s| s.avg_median_tolerance = 1.0));
        assert!(bad(|s| s.avg_min_km_per_liter = s.avg_max_km_per_liter));
        assert!(bad(|s| s.reconciliation_volume_tolerance_ratio = 1.5));
        assert!(bad(|s| s.preventive_max_extension_km = f64::NAN));
        assert!(bad(|s| s.preventive_attention_km_margin = -1.0));
        assert!(bad(|s| s.garage_base_latitude = Some(91.0)));
        assert!(bad(|s| s.garage_base_longitude = Some(181.0)));
        assert!(bad(|s| s.garage_base_latitude = Some(-23.0)));
        assert!(bad(|s| s.garage_base_radius_meters = f64::INFINITY));
        assert!(bad(|s| s.garage_cycle_fallback_distance_km = 0.0));
        assert!(bad(|s| s.garage_odometer_max_single_jump_km = f64::NAN));
    }
}
