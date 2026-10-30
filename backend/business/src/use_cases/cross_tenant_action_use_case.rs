use crate::domain::business_error::BusinessError;
use crate::domain::cross_tenant_action::CrossTenantAction;
use crate::gateway::cross_tenant_action_gateway::CrossTenantActionStore;

/// `C-036` (`HRMS-877`, `SR-AC-08-05`/`06`): the trail of what a tenant did inside its descendants.
pub struct CrossTenantActionUseCase<S: CrossTenantActionStore> {
    store: S,
}

impl<S: CrossTenantActionStore> CrossTenantActionUseCase<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Records the action before it is carried out: if the record cannot be written the action does not run.
    pub async fn record(&self, action: CrossTenantAction) -> Result<(), BusinessError> {
        log::info!(
            "[CrossTenantActionUseCase::record] {} {} from tenant {} into tenant {}",
            action.method, action.path, action.actor_tenant_id, action.affected_tenant_id
        );
        self.store.record(action).await
    }

    /// What `tenant_id` did, and what was done in it, newest first.
    pub async fn find_page(&self, tenant_id: i64, page: u64, page_size: u64) -> Result<(Vec<CrossTenantAction>, u64), BusinessError> {
        log::info!("[CrossTenantActionUseCase::find_page] Executing for tenant: {}", tenant_id);
        self.store.list_page(tenant_id, page, page_size).await
    }
}
