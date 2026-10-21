use business::domain::business_error::BusinessError;
use business::domain::inspection_model::InspectionModel;
use business::domain::inspection_model_item::InspectionModelItem;
use business::gateway::inspection_model_gateway::InspectionModelStore;
use business::gateway::inspection_model_item_gateway::InspectionModelItemStore;
use business::use_cases::inspection_model_use_case::{
    DUPLICATE_NAME, ITEMS_REQUIRED, InspectionModelUseCase, MODEL_NOT_FOUND, NAME_REQUIRED,
    PERIODICITY_INVALID,
};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct MemoryInspectionModelStore(Arc<Mutex<Vec<InspectionModel>>>);

#[async_trait]
impl InspectionModelStore for MemoryInspectionModelStore {
    async fn save(&self, mut model: InspectionModel) -> Result<InspectionModel, BusinessError> {
        let mut models = self
            .0
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let id = model.id.unwrap_or(models.len() as i64 + 1);
        model.id = Some(id);
        model.uuid = Some(format!("inspection-model-{id}"));
        models.push(model.clone());
        Ok(model)
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<InspectionModel>, BusinessError> {
        let models = self
            .0
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(models
            .iter()
            .find(|model| model.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }

    async fn find_by_name(
        &self,
        name: &str,
        tenant_id: Option<i64>,
    ) -> Result<Option<InspectionModel>, BusinessError> {
        let models = self
            .0
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(models
            .iter()
            .find(|model| model.name == name && model.tenant_id == tenant_id)
            .cloned())
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<InspectionModel>, u64), BusinessError> {
        let models = self
            .0
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let total = models.len() as u64;
        let start = (page * page_size) as usize;
        Ok((
            models
                .iter()
                .skip(start)
                .take(page_size as usize)
                .cloned()
                .collect(),
            total,
        ))
    }
}

#[derive(Clone, Default)]
struct MemoryInspectionModelItemStore(Arc<Mutex<Vec<InspectionModelItem>>>);

#[async_trait]
impl InspectionModelItemStore for MemoryInspectionModelItemStore {
    async fn save(
        &self,
        mut item: InspectionModelItem,
    ) -> Result<InspectionModelItem, BusinessError> {
        let mut items = self
            .0
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        item.id = Some(items.len() as i64 + 1);
        item.uuid = Some(format!("inspection-item-{}", item.id.unwrap_or_default()));
        items.push(item.clone());
        Ok(item)
    }

    async fn list_by_model(
        &self,
        model_id: i64,
    ) -> Result<Vec<InspectionModelItem>, BusinessError> {
        let items = self
            .0
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(items
            .iter()
            .filter(|item| item.inspection_model_id == model_id)
            .cloned()
            .collect())
    }
}

fn model(name: &str) -> InspectionModel {
    InspectionModel {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        name: name.into(),
        generates_work_order: false,
        periodicity_days: Some(90),
        observation: None,
        active: true,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn use_case() -> InspectionModelUseCase {
    use_case_with_stores(
        MemoryInspectionModelStore::default(),
        MemoryInspectionModelItemStore::default(),
    )
}

fn use_case_with_stores(
    models: MemoryInspectionModelStore,
    items: MemoryInspectionModelItemStore,
) -> InspectionModelUseCase {
    InspectionModelUseCase::new(models, items)
}

#[tokio::test]
async fn inspection_model_creation_saves_clean_items_linked_to_the_persisted_model() {
    let use_case = use_case();

    let (created, items) = use_case
        .create(
            model("  Annual inspection  "),
            vec!["  Brakes  ".into(), " ".into(), "Lights".into()],
        )
        .await
        .expect("model and its nonblank items are stored");
    let (page, total) = use_case.find_page(0, 10).await.expect("models are listed");

    assert_eq!(created.name, "Annual inspection");
    assert_eq!(items.len(), 2);
    assert!(
        items
            .iter()
            .all(|item| item.inspection_model_id == created.id.unwrap_or_default())
    );
    assert_eq!(total, 1);
    assert_eq!(page[0].1.len(), 2);
}

#[tokio::test]
async fn inspection_model_validation_and_duplicate_checks_run_before_item_writes() {
    let use_case = use_case();
    assert_eq!(
        use_case
            .create(model("  "), vec!["item".into()])
            .await
            .expect_err("blank model name is rejected")
            .message,
        NAME_REQUIRED
    );
    assert_eq!(
        use_case
            .create(model("Valid"), vec![" ".into()])
            .await
            .expect_err("at least one item is required")
            .message,
        ITEMS_REQUIRED
    );
    use_case
        .create(model("Annual inspection"), vec!["Brakes".into()])
        .await
        .expect("first model is stored");
    assert_eq!(
        use_case
            .create(model("Annual inspection"), vec!["Lights".into()])
            .await
            .expect_err("duplicate model name is rejected")
            .message,
        DUPLICATE_NAME
    );
}

#[tokio::test]
async fn inspection_models_can_be_loaded_with_items_and_missing_models_are_reported() {
    let models = MemoryInspectionModelStore::default();
    let items = MemoryInspectionModelItemStore::default();
    let use_case = use_case_with_stores(models, items);
    let (created, created_items) = use_case
        .create(model("Annual inspection"), vec!["Brakes".into()])
        .await
        .unwrap();

    let (found, found_items) = use_case
        .find_by_uuid(created.uuid.clone().unwrap())
        .await
        .expect("model and its items can be loaded together");
    assert_eq!(found, created);
    assert_eq!(found_items, created_items);
    assert_eq!(
        use_case
            .find_by_uuid("missing".into())
            .await
            .expect_err("unknown model is not found")
            .message,
        MODEL_NOT_FOUND
    );
}

#[tokio::test]
async fn inspection_models_reject_non_positive_periodicity() {
    let use_case = use_case();
    let mut invalid = model("Annual inspection");
    invalid.periodicity_days = Some(0);

    assert_eq!(
        use_case
            .create(invalid, vec!["Brakes".into()])
            .await
            .expect_err("periodicity must be positive")
            .message,
        PERIODICITY_INVALID
    );
}
