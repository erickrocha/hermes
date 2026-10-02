use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::bytes_para_string;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::fuel_gauge_setting::{FuelGaugeSetting, FuelGaugeSettingEntityMapper};
use crate::gateway::fuel_gauge_setting_gateway::FuelGaugeSettingGateway;
use crate::use_cases::fuel_gauge_use_case::GaugeSettings;
use sea_orm::DbErr;

pub const SUSPECT_MARGIN_INVALID: &str = "The suspect margin must be at least 1 (a share of the tank's capacity)";
pub const SET_ASIDE_FLOOR_INVALID: &str = "The set-aside litres floor cannot be negative";
pub const RATIO_INVALID: &str = "The set-aside and large-fuelling shares must be greater than 0 and at most 1";

/// `EPIC-FU-06-S03` (`HRMS-953`): a tenant's tank-gauge thresholds.
pub struct FuelGaugeSettingUseCase {
    gateway: FuelGaugeSettingGateway,
}

impl FuelGaugeSettingUseCase {
    pub fn new(gateway: FuelGaugeSettingGateway) -> Self {
        Self { gateway }
    }

    /// The tenant's own thresholds, or the platform defaults when it has set none.
    pub async fn current(&self, target_tenant_id: Option<i64>) -> Result<GaugeSettings, BusinessError> {
        let row = self.gateway.find_current(target_tenant_id).await.map_err(database_error)?;
        Ok(row.map(|m| GaugeSettings::from(&FuelGaugeSettingEntityMapper::from_model(m))).unwrap_or_default())
    }

    /// Upserts the tenant's single row.
    pub async fn configure(&self, setting: FuelGaugeSetting) -> Result<FuelGaugeSetting, BusinessError> {
        if setting.suspect_margin_ratio.is_nan() || setting.suspect_margin_ratio < 1.0 {
            return Err(BusinessError::new(SUSPECT_MARGIN_INVALID.to_string()));
        }
        if setting.set_aside_min_expected_liters.is_nan() || setting.set_aside_min_expected_liters < 0.0 {
            return Err(BusinessError::new(SET_ASIDE_FLOOR_INVALID.to_string()));
        }
        for ratio in [setting.set_aside_ratio, setting.large_fuelling_ratio] {
            if ratio.is_nan() || ratio <= 0.0 || ratio > 1.0 {
                return Err(BusinessError::new(RATIO_INVALID.to_string()));
            }
        }
        let existing = self.gateway.find_current(setting.tenant_id).await.map_err(database_error)?;
        let setting = match existing {
            Some(model) => FuelGaugeSetting {
                id: Some(model.id),
                uuid: Some(bytes_para_string(model.uuid)),
                ..setting
            },
            None => setting,
        };
        let saved = self.gateway.persist(setting).await.map_err(database_error)?;
        Ok(FuelGaugeSettingEntityMapper::from_active_model(saved))
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[FuelGaugeSettingUseCase] {}", msg);
    BusinessError::new(msg)
}
