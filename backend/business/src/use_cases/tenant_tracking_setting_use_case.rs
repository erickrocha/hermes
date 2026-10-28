use crate::domain::business_error::BusinessError;
use crate::domain::tenant_tracking_setting::TrackingCredentials;
use crate::gateway::tenant_tracking_setting_gateway::TenantTrackingSettingStore;

pub const TRACKING_SETTING_INVALID: &str = "The tracking provider URL, e-mail and password are required";

/// `HRMS-900`..`905`: a tenant's GPS provider account.
pub struct TenantTrackingSettingUseCase<G: TenantTrackingSettingStore> {
    gateway: G,
}

impl<G: TenantTrackingSettingStore> TenantTrackingSettingUseCase<G> {
    pub fn new(gateway: G) -> Self {
        Self { gateway }
    }

    pub async fn current(&self, tenant_id: i64) -> Result<Option<TrackingCredentials>, BusinessError> {
        self.gateway.get(tenant_id).await
    }

    /// Upserts the tenant's account. A blank `password` keeps the stored one, so the console
    /// never has to be given the secret back to edit the URL or the e-mail.
    pub async fn configure(
        &self,
        tenant_id: i64,
        base_url: &str,
        email: &str,
        password: Option<&str>,
    ) -> Result<TrackingCredentials, BusinessError> {
        let base_url = base_url.trim();
        let email = email.trim();
        let kept = match password.map(str::trim).filter(|p| !p.is_empty()) {
            Some(password) => Some(password.to_string()),
            None => self.gateway.get(tenant_id).await?.map(|existing| existing.password),
        };
        let url_ok = base_url.starts_with("https://") || base_url.starts_with("http://");
        match kept {
            Some(password) if url_ok && !email.is_empty() => {
                let credentials = TrackingCredentials {
                    base_url: base_url.to_string(),
                    email: email.to_string(),
                    password,
                };
                self.gateway.save(tenant_id, credentials.clone()).await?;
                Ok(credentials)
            }
            _ => Err(BusinessError::new(TRACKING_SETTING_INVALID.to_string())),
        }
    }
}
