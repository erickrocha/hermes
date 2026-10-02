use crate::domain::business_error::BusinessError;
use crate::domain::checklist_template::ChecklistTemplate;
use crate::domain::checklist_template_item::ChecklistTemplateItem;
use crate::gateway::checklist_template_gateway::ChecklistTemplateStore;

pub const NAME_REQUIRED: &str = "Name is required";
pub const AT_LEAST_ONE_ITEM_REQUIRED: &str = "A checklist template needs at least one item";
pub const ITEM_DESCRIPTION_REQUIRED: &str = "Every item needs a description";
pub const TEMPLATE_NOT_FOUND: &str = "Checklist template not found";

pub struct ChecklistTemplateUseCase {
    gateway: Box<dyn ChecklistTemplateStore>,
}

impl ChecklistTemplateUseCase {
    pub fn new<G: ChecklistTemplateStore + 'static>(gateway: G) -> Self {
        Self {
            gateway: Box::new(gateway),
        }
    }

    /// `HRMS-651`: the template and its items are written together,
    /// all-or-nothing -- the same `PD-027` reasoning `ExtraTripUseCase::
    /// import` uses for a batch, applied here to one create with children.
    pub async fn create(
        &self,
        template: ChecklistTemplate,
        items: Vec<ChecklistTemplateItem>,
    ) -> Result<(ChecklistTemplate, Vec<ChecklistTemplateItem>), BusinessError> {
        let template = Self::validated(template, &items)?;

        self.gateway.create(template, items).await
    }

    fn validated(
        template: ChecklistTemplate,
        items: &[ChecklistTemplateItem],
    ) -> Result<ChecklistTemplate, BusinessError> {
        if template.name.trim().is_empty() {
            return Err(BusinessError::new(NAME_REQUIRED.to_string()));
        }
        if items.is_empty() {
            return Err(BusinessError::new(AT_LEAST_ONE_ITEM_REQUIRED.to_string()));
        }
        if items.iter().any(|item| item.description.trim().is_empty()) {
            return Err(BusinessError::new(ITEM_DESCRIPTION_REQUIRED.to_string()));
        }
        Ok(template)
    }

    pub async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<(ChecklistTemplate, Vec<ChecklistTemplateItem>), BusinessError> {
        let template = match self.gateway.get_by_uuid(uuid).await? {
            Some(template) => template,
            None => return Err(BusinessError::new(TEMPLATE_NOT_FOUND.to_string())),
        };
        let items = self
            .gateway
            .list_items(template.id.unwrap_or_default())
            .await?;
        Ok((template, items))
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<ChecklistTemplate>, u64), BusinessError> {
        self.gateway.list_page(page, page_size).await
    }
}
