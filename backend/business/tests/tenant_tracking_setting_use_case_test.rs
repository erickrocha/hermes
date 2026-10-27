use business::domain::business_error::BusinessError;
use business::domain::tenant_tracking_setting::TrackingCredentials;
use business::gateway::tenant_tracking_setting_gateway::TenantTrackingSettingStore;
use business::use_cases::tenant_tracking_setting_use_case::TenantTrackingSettingUseCase;
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryStore {
    row: Mutex<Option<TrackingCredentials>>,
}

#[async_trait]
impl TenantTrackingSettingStore for MemoryStore {
    async fn get(&self, _tenant_id: i64) -> Result<Option<TrackingCredentials>, BusinessError> {
        Ok(self.row.lock().expect("lock").clone())
    }

    async fn save(&self, _tenant_id: i64, credentials: TrackingCredentials) -> Result<(), BusinessError> {
        *self.row.lock().expect("lock") = Some(credentials);
        Ok(())
    }
}

const URL: &str = "https://api.pinme.io/api";

#[tokio::test]
async fn the_first_configuration_needs_a_password() {
    let use_case = TenantTrackingSettingUseCase::new(MemoryStore::default());
    assert!(use_case.configure(1, URL, "a@b.c", None).await.is_err());
    assert!(use_case.configure(1, URL, "a@b.c", Some("  ")).await.is_err());
    assert!(use_case.current(1).await.expect("read").is_none());
}

#[tokio::test]
async fn a_blank_password_keeps_the_stored_one() {
    let use_case = TenantTrackingSettingUseCase::new(MemoryStore::default());
    use_case.configure(1, URL, "a@b.c", Some("secret")).await.expect("first save");
    use_case.configure(1, "http://other.example/api", "x@y.z", None).await.expect("edit without password");
    let saved = use_case.current(1).await.expect("read").expect("row");
    assert_eq!((saved.base_url.as_str(), saved.email.as_str(), saved.password.as_str()), ("http://other.example/api", "x@y.z", "secret"));
}

#[tokio::test]
async fn a_url_without_scheme_or_a_blank_email_is_refused() {
    let use_case = TenantTrackingSettingUseCase::new(MemoryStore::default());
    assert!(use_case.configure(1, "api.pinme.io", "a@b.c", Some("secret")).await.is_err());
    assert!(use_case.configure(1, URL, " ", Some("secret")).await.is_err());
}

#[test]
fn the_password_never_reaches_a_debug_line() {
    let credentials = TrackingCredentials { base_url: URL.into(), email: "a@b.c".into(), password: "hunter2".into() };
    assert!(!format!("{credentials:?}").contains("hunter2"));
}
