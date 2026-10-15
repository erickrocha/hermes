use business::domain::business_error::BusinessError;
use business::domain::part::Part;
use business::gateway::part_gateway::PartStore;
use business::use_cases::part_use_case::PartUseCase;
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryPartStore {
    parts: Mutex<Vec<Part>>,
}

#[async_trait]
impl PartStore for MemoryPartStore {
    async fn save(&self, mut part: Part) -> Result<Part, BusinessError> {
        let mut parts = self
            .parts
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let id = part.id.unwrap_or(parts.len() as i64 + 1);
        part.id = Some(id);
        part.uuid = Some(format!("part-{id}"));
        parts.push(part.clone());
        Ok(part)
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<Part>, BusinessError> {
        let parts = self
            .parts
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(parts
            .iter()
            .find(|part| part.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<Part>, u64), BusinessError> {
        let parts = self
            .parts
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let total = parts.len() as u64;
        let start = (page * page_size) as usize;
        Ok((
            parts
                .iter()
                .skip(start)
                .take(page_size as usize)
                .cloned()
                .collect(),
            total,
        ))
    }
}

fn new_part() -> Part {
    Part {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        name: "  Oil filter  ".into(),
        category: Some("Filters".into()),
        application: None,
        minimum_stock: 2.0,
        unit: "  unit  ".into(),
        unit_value_cents: Some(2_500),
        default_supplier: None,
        location: None,
        observation: None,
        moving_average_cost_cents: None,
        last_purchase_price_cents: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn part_use_case_uses_its_store_port_without_a_database_gateway() {
    let use_case = PartUseCase::new(MemoryPartStore::default());

    let created = use_case
        .create(new_part())
        .await
        .expect("valid part is stored");
    let found = use_case
        .find_by_uuid(created.uuid.clone().expect("store assigned uuid"))
        .await
        .expect("part is found");
    let (page, total) = use_case.find_page(0, 10).await.expect("page is listed");

    assert_eq!(created.name, "Oil filter");
    assert_eq!(created.unit, "unit");
    assert_eq!(found.id, created.id);
    assert_eq!(total, 1);
    assert_eq!(page.len(), 1);
}

#[tokio::test]
async fn part_use_case_rejects_negative_minimum_stock_without_storage() {
    let use_case = PartUseCase::new(MemoryPartStore::default());
    let mut invalid = new_part();
    invalid.minimum_stock = -1.0;

    let error = use_case
        .create(invalid)
        .await
        .expect_err("negative minimum stock is rejected");

    assert_eq!(error.message, "A part's minimum stock cannot be negative");
}
