use business::domain::business_error::BusinessError;
use business::domain::tenant::Tenant;
use business::gateway::tenant_gateway::TenantStore;
use business::use_cases::tenant_use_case::TenantUseCase;
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryTenantStore {
    tenants: Mutex<Vec<Tenant>>,
}

#[async_trait]
impl TenantStore for MemoryTenantStore {
    async fn save(&self, mut tenant: Tenant) -> Result<Tenant, BusinessError> {
        let mut tenants = self.tenants.lock().map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        match tenant.id {
            Some(id) => {
                if let Some(existing) = tenants.iter_mut().find(|existing| existing.id == Some(id)) {
                    *existing = tenant.clone();
                } else {
                    tenants.push(tenant.clone());
                }
            }
            None => {
                tenant.id = Some(tenants.len() as i64 + 1);
                tenant.uuid = Some(format!("tenant-{}", tenant.id.unwrap_or_default()));
                tenants.push(tenant.clone());
            }
        }
        Ok(tenant)
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<Tenant>, BusinessError> {
        let tenants = self.tenants.lock().map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(tenants.iter().find(|tenant| tenant.id == Some(id)).cloned())
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<Tenant>, BusinessError> {
        let tenants = self.tenants.lock().map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(tenants.iter().find(|tenant| tenant.uuid.as_deref() == Some(&uuid)).cloned())
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<Tenant>, u64), BusinessError> {
        let tenants = self.tenants.lock().map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let filtered: Vec<_> = tenants
            .iter()
            .filter(|tenant| search.is_none_or(|query| tenant.business_name.contains(query)))
            .cloned()
            .collect();
        let total = filtered.len() as u64;
        let start = (page * page_size) as usize;
        Ok((filtered.into_iter().skip(start).take(page_size as usize).collect(), total))
    }

    async fn list_all(&self) -> Result<Vec<Tenant>, BusinessError> {
        let tenants = self.tenants.lock().map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(tenants.clone())
    }
}

fn new_tenant() -> Tenant {
    Tenant {
        id: None,
        uuid: None,
        business_name: "Example tenant".into(),
        company_name: None,
        tax_id: "12-3456789".into(),
        email: None,
        phone: None,
        website: None,
        address_line1: None,
        address_line2: None,
        locality: None,
        administrative_area: None,
        postal_code: None,
        country_code: Some("US".into()),
        business_plan_id: Some(99),
        created_at: None,
        updated_at: None,
        created_by: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn tenant_use_case_uses_its_store_port_without_a_database_gateway() {
    let use_case = TenantUseCase::new(MemoryTenantStore::default());

    let created = use_case.create(new_tenant()).await.expect("valid tenant is stored");

    assert_eq!(created.id, Some(1));
    assert_eq!(created.tax_id, "12-3456789");
    assert_eq!(created.business_plan_id, None);
}

#[tokio::test]
async fn tenant_use_case_returns_validation_errors_without_calling_storage() {
    let use_case = TenantUseCase::new(MemoryTenantStore::default());
    let mut invalid = new_tenant();
    invalid.country_code = None;

    let error = use_case.create(invalid).await.expect_err("country is required");

    assert_eq!(error.message, "Country code must contain two letters");
}