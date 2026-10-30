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
        parent_tenant_id: Some(1),
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
/// Masks are presentation, not data: the postal code and the phone are stored digits only, and a value
/// with no digit is stored as absent -- on create and on update alike.
#[tokio::test]
async fn tenant_postal_code_and_phone_are_stored_as_digits() {
    let use_case = TenantUseCase::new(MemoryTenantStore::default());
    let created = use_case
        .create(Tenant {
            phone: Some("+1 (415) 555-0199".into()),
            postal_code: Some("94105-1234".into()),
            ..new_tenant()
        })
        .await
        .expect("stored");
    assert_eq!(created.phone.as_deref(), Some("14155550199"));
    assert_eq!(created.postal_code.as_deref(), Some("941051234"));

    let updated = use_case
        .update(
            created.id.expect("id"),
            Tenant { phone: Some(" - ".into()), postal_code: Some("13000-000".into()), ..new_tenant() },
        )
        .await
        .expect("updated");
    assert_eq!(updated.phone, None);
    assert_eq!(updated.postal_code.as_deref(), Some("13000000"));
}

/// `C-036` (`HRMS-872`): every tenant but the root has a parent, so a creation without one is refused; the
/// root is the one tenant with none, and an update keeps a tenant where it is in the tree.
#[tokio::test]
async fn a_tenant_needs_a_parent_the_root_is_the_one_without_and_an_update_keeps_the_place() {
    let store = MemoryTenantStore::default();
    store
        .tenants
        .lock()
        .expect("lock")
        .push(Tenant { id: Some(1), uuid: Some("root".into()), parent_tenant_id: None, ..new_tenant() });
    let use_case = TenantUseCase::new(store);

    let orphan = use_case.create(Tenant { parent_tenant_id: None, ..new_tenant() }).await;
    assert_eq!(orphan.expect_err("a parent is required").message, "Tenant parent is required");

    let child = use_case.create(new_tenant()).await.expect("created under the root");
    assert_eq!(child.parent_tenant_id, Some(1));
    assert_eq!(use_case.find_root().await.expect("root").id, Some(1));

    let updated = use_case
        .update(child.id.expect("id"), Tenant { parent_tenant_id: Some(77), ..new_tenant() })
        .await
        .expect("updated");
    assert_eq!(updated.parent_tenant_id, Some(1), "a payload cannot move a tenant in the tree");
}

/// `C-036` (`HRMS-873`): a branch is the tenant and everything below it, the tenant first.
#[tokio::test]
async fn a_branch_is_the_tenant_then_everything_below_it() {
    let store = MemoryTenantStore::default();
    {
        let mut tenants = store.tenants.lock().expect("lock");
        for (id, parent) in [(1, None), (2, Some(1)), (3, Some(2)), (4, Some(1))] {
            tenants.push(Tenant { id: Some(id), uuid: Some(format!("t{id}")), parent_tenant_id: parent, business_name: format!("n{id}"), ..new_tenant() });
        }
    }
    let use_case = TenantUseCase::new(store);
    let ids = |v: Vec<Tenant>| v.into_iter().filter_map(|t| t.id).collect::<Vec<_>>();
    assert_eq!(ids(use_case.list_branch(2).await.expect("branch")), vec![2, 3]);
    assert_eq!(ids(use_case.list_branch(1).await.expect("branch")), vec![1, 2, 3, 4]);
    assert_eq!(ids(use_case.list_branch(4).await.expect("branch")), vec![4]);
}

/// `C-036` (`SR-AC-08-13`): a caller moves tenants within their own branch -- never their own tenant, never out of
/// the branch, never into a cycle -- and the root stays the one without a parent.
#[tokio::test]
async fn a_tenant_moves_within_the_callers_branch_and_nowhere_else() {
    let store = MemoryTenantStore::default();
    {
        // root(1) -> A(2) -> A1(3), A2(4); root -> B(5)
        let mut tenants = store.tenants.lock().expect("lock");
        for (id, parent) in [(1, None), (2, Some(1)), (3, Some(2)), (4, Some(2)), (5, Some(1))] {
            tenants.push(Tenant { id: Some(id), uuid: Some(format!("t{id}")), parent_tenant_id: parent, business_name: format!("n{id}"), ..new_tenant() });
        }
    }
    let use_case = TenantUseCase::new(store);
    assert_eq!(use_case.move_tenant(4, 3, 2).await.expect("A2 under A1, from A").parent_tenant_id, Some(3));
    assert_eq!(use_case.move_tenant(3, 2, 2).await.expect("A1 back under A").parent_tenant_id, Some(2));
    for (tenant, parent, origin) in [(2, 3, 2), (5, 3, 2), (3, 5, 2), (2, 4, 1)] {
        let refused = use_case.move_tenant(tenant, parent, origin).await.expect_err("outside the branch, or a cycle");
        assert!(refused.message.starts_with("A tenant moves only") || refused.message.starts_with("The move is not valid"), "{}", refused.message);
    }
    assert!(use_case.move_tenant(1, 5, 1).await.expect_err("the root never moves").message.starts_with("A tenant moves only"));
}
