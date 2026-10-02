use business::domain::business_error::BusinessError;
use business::domain::service_type::ServiceType;
use business::gateway::service_type_gateway::ServiceTypeStore;
use business::use_cases::service_type_use_case::ServiceTypeUseCase;
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryServiceTypeStore {
    entries: Mutex<Vec<ServiceType>>,
}

#[async_trait]
impl ServiceTypeStore for MemoryServiceTypeStore {
    async fn save(&self, mut entry: ServiceType) -> Result<ServiceType, BusinessError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let id = entry.id.unwrap_or(entries.len() as i64 + 1);
        entry.id = Some(id);
        entry.uuid = Some(format!("service-type-{id}"));
        entries.push(entry.clone());
        Ok(entry)
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<ServiceType>, BusinessError> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(entries
            .iter()
            .find(|entry| entry.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }

    async fn get_by_code(
        &self,
        code: &str,
        tenant_id: Option<i64>,
    ) -> Result<Option<ServiceType>, BusinessError> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(entries
            .iter()
            .find(|entry| entry.code == code && entry.tenant_id == tenant_id)
            .cloned())
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<ServiceType>, u64), BusinessError> {
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

fn new_service_type() -> ServiceType {
    ServiceType {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        code: "  PARTS  ".into(),
        name: "  Parts and supplies  ".into(),
        category: None,
        active: true,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn service_type_use_case_uses_its_store_port_without_a_database_gateway() {
    let use_case = ServiceTypeUseCase::new(MemoryServiceTypeStore::default());

    let created = use_case
        .create(new_service_type())
        .await
        .expect("valid service type is stored");
    let found = use_case
        .find_by_uuid(created.uuid.clone().expect("store assigned uuid"))
        .await
        .expect("service type is found");
    let (page, total) = use_case.find_page(0, 10).await.expect("page is listed");

    assert_eq!(created.code, "PARTS");
    assert_eq!(created.name, "Parts and supplies");
    assert_eq!(found.id, created.id);
    assert_eq!(total, 1);
    assert_eq!(page.len(), 1);
}

#[tokio::test]
async fn service_type_use_case_rejects_duplicate_codes_through_the_store_port() {
    let use_case = ServiceTypeUseCase::new(MemoryServiceTypeStore::default());
    use_case
        .create(new_service_type())
        .await
        .expect("first service type is stored");

    let mut duplicate = new_service_type();
    duplicate.name = "Another name".into();
    let error = use_case
        .create(duplicate)
        .await
        .expect_err("duplicate code is rejected");

    assert_eq!(
        error.message,
        "This tenant already has a service type with this code"
    );
}
