use business::domain::business_error::BusinessError;
use business::domain::priced_service::PricedService;
use business::gateway::priced_service_gateway::PricedServiceStore;
use business::use_cases::priced_service_use_case::PricedServiceUseCase;
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryPricedServiceStore {
    entries: Mutex<Vec<PricedService>>,
}

#[async_trait]
impl PricedServiceStore for MemoryPricedServiceStore {
    async fn save(&self, mut entry: PricedService) -> Result<PricedService, BusinessError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let id = entry.id.unwrap_or(entries.len() as i64 + 1);
        entry.id = Some(id);
        entry.uuid = Some(format!("priced-service-{id}"));
        entries.push(entry.clone());
        Ok(entry)
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<PricedService>, BusinessError> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(entries
            .iter()
            .find(|entry| entry.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<PricedService>, u64), BusinessError> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let total = entries.len() as u64;
        let start = (page * page_size) as usize;
        Ok((
            entries
                .iter()
                .skip(start)
                .take(page_size as usize)
                .cloned()
                .collect(),
            total,
        ))
    }
}

fn new_priced_service() -> PricedService {
    PricedService {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        name: "  Oil change  ".into(),
        category: Some("Maintenance".into()),
        default_value_cents: Some(12_500),
        observation: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn priced_service_use_case_uses_its_store_port_without_a_database_gateway() {
    let use_case = PricedServiceUseCase::new(MemoryPricedServiceStore::default());

    let created = use_case
        .create(new_priced_service())
        .await
        .expect("valid priced service is stored");
    let found = use_case
        .find_by_uuid(created.uuid.clone().expect("store assigned uuid"))
        .await
        .expect("priced service is found");
    let (page, total) = use_case.find_page(0, 10).await.expect("page is listed");

    assert_eq!(created.name, "Oil change");
    assert_eq!(found.id, created.id);
    assert_eq!(total, 1);
    assert_eq!(page.len(), 1);
}

#[tokio::test]
async fn priced_service_use_case_rejects_a_blank_name_without_storage() {
    let use_case = PricedServiceUseCase::new(MemoryPricedServiceStore::default());
    let mut invalid = new_priced_service();
    invalid.name = "  ".into();

    let error = use_case
        .create(invalid)
        .await
        .expect_err("blank name is rejected");

    assert_eq!(error.message, "A priced service needs a name");
}
