use crate::domain::business_error::BusinessError;
use crate::domain::fuel_gauge_setting::FuelGaugeSetting;
use crate::gateway::fuel_gauge_setting_gateway::FuelGaugeSettingStore;
use crate::use_cases::fuel_gauge_use_case::GaugeSettings;

pub const SUSPECT_MARGIN_INVALID: &str =
    "The suspect margin must be at least 1 (a share of the tank's capacity)";
pub const SET_ASIDE_FLOOR_INVALID: &str = "The set-aside litres floor cannot be negative";
pub const RATIO_INVALID: &str =
    "The set-aside and large-fuelling shares must be greater than 0 and at most 1";

/// `EPIC-FU-06-S03` (`HRMS-953`): a tenant's tank-gauge thresholds.
pub struct FuelGaugeSettingUseCase<G: FuelGaugeSettingStore> {
    gateway: G,
}

impl<G: FuelGaugeSettingStore> FuelGaugeSettingUseCase<G> {
    pub fn new(gateway: G) -> Self {
        Self { gateway }
    }

    /// The tenant's own thresholds, or the platform defaults when it has set none.
    pub async fn current(
        &self,
        target_tenant_id: Option<i64>,
    ) -> Result<GaugeSettings, BusinessError> {
        let row = self.gateway.get_current(target_tenant_id).await?;
        Ok(row.as_ref().map(GaugeSettings::from).unwrap_or_default())
    }

    /// Upserts the tenant's single row.
    pub async fn configure(
        &self,
        setting: FuelGaugeSetting,
    ) -> Result<FuelGaugeSetting, BusinessError> {
        if setting.suspect_margin_ratio.is_nan() || setting.suspect_margin_ratio < 1.0 {
            return Err(BusinessError::new(SUSPECT_MARGIN_INVALID.to_string()));
        }
        if setting.set_aside_min_expected_liters.is_nan()
            || setting.set_aside_min_expected_liters < 0.0
        {
            return Err(BusinessError::new(SET_ASIDE_FLOOR_INVALID.to_string()));
        }
        for ratio in [setting.set_aside_ratio, setting.large_fuelling_ratio] {
            if ratio.is_nan() || ratio <= 0.0 || ratio > 1.0 {
                return Err(BusinessError::new(RATIO_INVALID.to_string()));
            }
        }
        let existing = self.gateway.get_current(setting.tenant_id).await?;
        let setting = match existing {
            Some(existing) => FuelGaugeSetting {
                id: existing.id,
                uuid: existing.uuid,
                ..setting
            },
            None => setting,
        };
        self.gateway.save(setting).await
    }
}
