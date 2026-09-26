use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::checklist_template::{ChecklistTemplate, ChecklistTemplateEntityMapper};
use crate::domain::checklist_template_item::{
    ChecklistTemplateItem, ChecklistTemplateItemEntityMapper,
};
use crate::gateway::checklist_template_gateway::ChecklistTemplateGateway;
use crate::gateway::checklist_template_item_gateway::ChecklistTemplateItemGateway;
use sea_orm::{ActiveModelTrait, DbErr, TransactionTrait};

pub const NAME_REQUIRED: &str = "Name is required";
pub const AT_LEAST_ONE_ITEM_REQUIRED: &str = "A checklist template needs at least one item";
pub const ITEM_DESCRIPTION_REQUIRED: &str = "Every item needs a description";
pub const TEMPLATE_NOT_FOUND: &str = "Checklist template not found";

pub struct ChecklistTemplateUseCase {
    gateway: ChecklistTemplateGateway,
    items: ChecklistTemplateItemGateway,
}

impl ChecklistTemplateUseCase {
    pub fn new(gateway: ChecklistTemplateGateway, items: ChecklistTemplateItemGateway) -> Self {
        Self { gateway, items }
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

        let transaction = self.gateway.db().begin().await.map_err(database_error)?;

        let saved_template = match ChecklistTemplateEntityMapper::build_active_model(template)
            .save(&transaction)
            .await
        {
            Ok(active) => ChecklistTemplateEntityMapper::from_active_model(active),
            Err(e) => {
                let _ = transaction.rollback().await;
                return Err(database_error(e));
            }
        };

        let mut saved_items = Vec::with_capacity(items.len());
        for mut item in items {
            item.checklist_template_id = saved_template.id.unwrap_or_default();
            match ChecklistTemplateItemEntityMapper::build_active_model(item)
                .save(&transaction)
                .await
            {
                Ok(active) => {
                    saved_items.push(ChecklistTemplateItemEntityMapper::from_active_model(active))
                }
                Err(e) => {
                    let _ = transaction.rollback().await;
                    return Err(database_error(e));
                }
            }
        }

        transaction.commit().await.map_err(database_error)?;
        Ok((saved_template, saved_items))
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
        let entity = self
            .gateway
            .find_by_uuid(uuid)
            .await
            .map_err(database_error)?;
        let template = match entity {
            Some(model) => ChecklistTemplateEntityMapper::from_model(model),
            None => return Err(BusinessError::new(TEMPLATE_NOT_FOUND.to_string())),
        };
        let items = self
            .items
            .find_by_template(template.id.unwrap_or_default())
            .await
            .map_err(database_error)?;
        Ok((template, ChecklistTemplateItemEntityMapper::from_models(items)))
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<ChecklistTemplate>, u64), BusinessError> {
        let (rows, total) = self
            .gateway
            .find_page(page, page_size)
            .await
            .map_err(database_error)?;
        Ok((ChecklistTemplateEntityMapper::from_models(rows), total))
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[ChecklistTemplateUseCase] {}", msg);
    BusinessError::new(msg)
}
