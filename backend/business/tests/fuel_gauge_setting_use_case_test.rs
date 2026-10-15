use business::domain::business_error::BusinessError;
use business::domain::fuel_gauge_setting::FuelGaugeSetting;
use business::gateway::fuel_gauge_setting_gateway::FuelGaugeSettingStore;
use business::use_cases::fuel_gauge_setting_use_case::{
    FuelGaugeSettingUseCase, RATIO_INVALID, SUSPECT_MARGIN_INVALID,
};
use business::use_cases::fuel_gauge_use_case::GaugeSettings;
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryFuelGaugeSettingStore {
    setting: Mutex<Option<FuelGaugeSetting>>,
}

#[async_trait]
impl FuelGaugeSettingStore for MemoryFuelGaugeSettingStore {
    async fn get_current(
        &self,
        target_tenant_id: Option<i64>,
    ) -> Result<Option<FuelGaugeSetting>, BusinessError> {
        let setting = self
            .setting
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(setting
            .as_ref()
            .filter(|setting| target_tenant_id.is_none_or(|id| setting.tenant_id == Some(id)))
            .cloned())
    }

    async fn save(&self, mut setting: FuelGaugeSetting) -> Result<FuelGaugeSetting, BusinessError> {
        let mut stored = self
            .setting
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        if let Some(existing) = stored.as_ref()
            && existing.tenant_id == setting.tenant_id
        {
            setting.id = existing.id;
            setting.uuid = existing.uuid.clone();
        }
        setting.id.get_or_insert(1);
        setting
            .uuid
            .get_or_insert_with(|| "fuel-gauge-setting-1".into());
        *stored = Some(setting.clone());
        Ok(setting)
    }
}

fn setting() -> FuelGaugeSetting {
    FuelGaugeSetting {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        suspect_margin_ratio: 1.2,
        set_aside_min_expected_liters: 12.0,
        set_aside_ratio: 0.3,
        large_fuelling_ratio: 0.4,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn gauge_settings_fall_back_to_defaults_until_a_tenant_configures_them() {
    let use_case = FuelGaugeSettingUseCase::new(MemoryFuelGaugeSettingStore::default());

    assert_eq!(
        use_case
            .current(Some(42))
            .await
            .expect("defaults are available"),
        GaugeSettings::default()
    );

    let configured = use_case
        .configure(setting())
        .await
        .expect("setting is saved");
    let current = use_case.current(Some(42)).await.expect("setting is loaded");

    assert_eq!(configured.id, Some(1));
    assert_eq!(current.suspect_margin_ratio, 1.2);
    assert_eq!(current.set_aside_ratio, 0.3);
}

#[tokio::test]
async fn configuring_again_preserves_the_tenant_settings_identity() {
    let use_case = FuelGaugeSettingUseCase::new(MemoryFuelGaugeSettingStore::default());
    let first = use_case
        .configure(setting())
        .await
        .expect("initial row is saved");
    let mut changed = setting();
    changed.set_aside_ratio = 0.6;

    let updated = use_case
        .configure(changed)
        .await
        .expect("existing row is updated");

    assert_eq!(updated.id, first.id);
    assert_eq!(updated.uuid, first.uuid);
    assert_eq!(updated.set_aside_ratio, 0.6);
}

#[tokio::test]
async fn invalid_gauge_settings_are_rejected_before_storage() {
    let use_case = FuelGaugeSettingUseCase::new(MemoryFuelGaugeSettingStore::default());
    let mut invalid_margin = setting();
    invalid_margin.suspect_margin_ratio = 0.9;
    let margin_error = use_case
        .configure(invalid_margin)
        .await
        .expect_err("margin below one is rejected");

    let mut invalid_ratio = setting();
    invalid_ratio.large_fuelling_ratio = 1.5;
    let ratio_error = use_case
        .configure(invalid_ratio)
        .await
        .expect_err("ratio above one is rejected");

    assert_eq!(margin_error.message, SUSPECT_MARGIN_INVALID);
    assert_eq!(ratio_error.message, RATIO_INVALID);
}
