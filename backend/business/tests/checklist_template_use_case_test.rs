use business::domain::business_error::BusinessError;
use business::domain::checklist_template::ChecklistTemplate;
use business::domain::checklist_template_item::ChecklistTemplateItem;
use business::domain::enums::ChecklistType;
use business::gateway::checklist_template_gateway::ChecklistTemplateStore;
use business::use_cases::checklist_template_use_case::{
    AT_LEAST_ONE_ITEM_REQUIRED, ChecklistTemplateUseCase, ITEM_DESCRIPTION_REQUIRED, NAME_REQUIRED,
};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};

struct MemoryChecklistTemplateStore {
    creates: AtomicUsize,
}

#[async_trait]
impl ChecklistTemplateStore for MemoryChecklistTemplateStore {
    async fn create(
        &self,
        template: ChecklistTemplate,
        items: Vec<ChecklistTemplateItem>,
    ) -> Result<(ChecklistTemplate, Vec<ChecklistTemplateItem>), BusinessError> {
        self.creates.fetch_add(1, Ordering::Relaxed);
        Ok((template, items))
    }

    async fn get_by_uuid(&self, _uuid: String) -> Result<Option<ChecklistTemplate>, BusinessError> {
        Ok(None)
    }

    async fn list_items(
        &self,
        _template_id: i64,
    ) -> Result<Vec<ChecklistTemplateItem>, BusinessError> {
        Ok(Vec::new())
    }

    async fn list_page(
        &self,
        _page: u64,
        _page_size: u64,
    ) -> Result<(Vec<ChecklistTemplate>, u64), BusinessError> {
        Ok((Vec::new(), 0))
    }
}

fn template(name: &str) -> ChecklistTemplate {
    ChecklistTemplate {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        name: name.into(),
        checklist_type: ChecklistType::Departure,
        active: true,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn item(description: &str) -> ChecklistTemplateItem {
    ChecklistTemplateItem {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        checklist_template_id: 0,
        description: description.into(),
        generates_work_order: false,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn template_validation_refuses_invalid_content_before_calling_the_store() {
    let store = MemoryChecklistTemplateStore {
        creates: AtomicUsize::new(0),
    };
    let use_case = ChecklistTemplateUseCase::new(store);

    let blank_name = use_case
        .create(template("  "), vec![item("Inspect brakes")])
        .await
        .expect_err("template name is required");
    let no_items = use_case
        .create(template("Departure"), Vec::new())
        .await
        .expect_err("at least one item is required");
    let blank_item = use_case
        .create(template("Departure"), vec![item("  ")])
        .await
        .expect_err("item description is required");

    assert_eq!(blank_name.message, NAME_REQUIRED);
    assert_eq!(no_items.message, AT_LEAST_ONE_ITEM_REQUIRED);
    assert_eq!(blank_item.message, ITEM_DESCRIPTION_REQUIRED);
}
