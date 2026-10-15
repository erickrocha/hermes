use business::domain::business_error::BusinessError;
use business::domain::tenant_rule_setting::{RuleSettings, TenantRuleSetting};
use business::gateway::tenant_rule_setting_gateway::TenantRuleSettingStore;
use business::use_cases::tenant_rule_setting_use_case::{
    SETTING_INVALID, TenantRuleSettingUseCase,
};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryTenantRuleSettingStore {
    setting: Mutex<Option<TenantRuleSetting>>,
}

#[async_trait]
impl TenantRuleSettingStore for MemoryTenantRuleSettingStore {
    async fn get_current(
        &self,
        target_tenant_id: Option<i64>,
    ) -> Result<Option<TenantRuleSetting>, BusinessError> {
        let setting = self
            .setting
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(setting
            .as_ref()
            .filter(|setting| target_tenant_id.is_none_or(|id| setting.tenant_id == Some(id)))
            .cloned())
    }

    async fn save(
        &self,
        mut setting: TenantRuleSetting,
    ) -> Result<TenantRuleSetting, BusinessError> {
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
            .get_or_insert_with(|| "tenant-rule-setting-1".into());
        *stored = Some(setting.clone());
        Ok(setting)
    }
}

fn setting(tenant_id: Option<i64>, settings: RuleSettings) -> TenantRuleSetting {
    TenantRuleSetting::new(tenant_id, settings)
}

#[tokio::test]
async fn rule_settings_use_defaults_until_a_tenant_configures_a_row() {
    let use_case = TenantRuleSettingUseCase::new(MemoryTenantRuleSettingStore::default());

    assert_eq!(
        use_case
            .current(Some(42))
            .await
            .expect("defaults are available"),
        RuleSettings::default()
    );

    let configured = use_case
        .configure(setting(Some(42), RuleSettings::default()))
        .await
        .expect("sensible rule settings are saved");
    let current = use_case
        .current(Some(42))
        .await
        .expect("settings are loaded");

    assert_eq!(configured.id, Some(1));
    assert_eq!(current, RuleSettings::default());
}

#[tokio::test]
async fn configuring_rule_settings_again_preserves_the_stored_identity() {
    let use_case = TenantRuleSettingUseCase::new(MemoryTenantRuleSettingStore::default());
    let first = use_case
        .configure(setting(Some(42), RuleSettings::default()))
        .await
        .expect("initial settings are saved");
    let changed = RuleSettings {
        avg_recent_window_days: 45,
        ..RuleSettings::default()
    };

    let updated = use_case
        .configure(setting(Some(42), changed))
        .await
        .expect("existing settings are updated");

    assert_eq!(updated.id, first.id);
    assert_eq!(updated.uuid, first.uuid);
    assert_eq!(updated.avg_recent_window_days, 45);
}

#[tokio::test]
async fn invalid_rule_settings_are_rejected_without_storage() {
    let use_case = TenantRuleSettingUseCase::new(MemoryTenantRuleSettingStore::default());
    let invalid = RuleSettings {
        avg_recent_window_days: 0,
        ..RuleSettings::default()
    };

    let error = use_case
        .configure(setting(Some(42), invalid))
        .await
        .expect_err("invalid settings are refused");

    assert_eq!(error.message, SETTING_INVALID);
    assert_eq!(
        use_case.current(Some(42)).await.expect("defaults remain"),
        RuleSettings::default()
    );
}

#[tokio::test]
async fn the_non_execution_settings_default_to_the_legacy_values_and_refuse_out_of_range_ones() {
    let defaults = RuleSettings::default();
    assert_eq!((defaults.non_execution_grace_minutes, defaults.non_execution_min_dwell_minutes), (60, 10));
    let use_case = TenantRuleSettingUseCase::new(MemoryTenantRuleSettingStore::default());
    let zero_grace = RuleSettings { non_execution_grace_minutes: 0, ..RuleSettings::default() };
    assert!(use_case.configure(setting(Some(42), zero_grace)).await.is_ok(), "a grace of 0 is allowed");
    for invalid in [
        RuleSettings { non_execution_grace_minutes: -1, ..RuleSettings::default() },
        RuleSettings { non_execution_min_dwell_minutes: 0, ..RuleSettings::default() },
    ] {
        let error = use_case.configure(setting(Some(42), invalid)).await.expect_err("refused");
        assert_eq!(error.message, SETTING_INVALID);
    }
}
