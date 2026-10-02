use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::inspection_model::{InspectionModel, InspectionModelEntityMapper};
use crate::domain::inspection_model_item::{InspectionModelItem, InspectionModelItemEntityMapper};
use crate::gateway::inspection_model_gateway::InspectionModelGateway;
use crate::gateway::inspection_model_item_gateway::InspectionModelItemGateway;
use sea_orm::DbErr;

pub const NAME_REQUIRED: &str = "An inspection model needs a name";
pub const ITEMS_REQUIRED: &str = "An inspection model needs at least one non-blank item";
pub const PERIODICITY_INVALID: &str = "The periodicity must be a positive number of days";
pub const DUPLICATE_NAME: &str = "This tenant already has an inspection model with this name";
pub const MODEL_NOT_FOUND: &str = "Inspection model not found";

/// `EPIC-MT-07-S10` (`HRMS-715`): technical-inspection templates.
pub struct InspectionModelUseCase {
    models: InspectionModelGateway,
    items: InspectionModelItemGateway,
}

impl InspectionModelUseCase {
    pub fn new(models: InspectionModelGateway, items: InspectionModelItemGateway) -> Self {
        Self { models, items }
    }

    pub async fn create(
        &self,
        model: InspectionModel,
        descriptions: Vec<String>,
    ) -> Result<(InspectionModel, Vec<InspectionModelItem>), BusinessError> {
        let name = model.name.trim().to_string();
        if name.is_empty() {
            return Err(BusinessError::new(NAME_REQUIRED.to_string()));
        }
        let descriptions: Vec<String> =
            descriptions.into_iter().map(|d| d.trim().to_string()).filter(|d| !d.is_empty()).collect();
        if descriptions.is_empty() {
            return Err(BusinessError::new(ITEMS_REQUIRED.to_string()));
        }
        if model.periodicity_days.is_some_and(|d| d <= 0) {
            return Err(BusinessError::new(PERIODICITY_INVALID.to_string()));
        }
        if self.models.find_by_name(&name, model.tenant_id).await.map_err(database_error)?.is_some() {
            return Err(BusinessError::new(DUPLICATE_NAME.to_string()));
        }

        let saved = self.models.persist(InspectionModel { name, ..model }).await.map_err(database_error)?;
        let saved = InspectionModelEntityMapper::from_active_model(saved);
        let mut items = Vec::with_capacity(descriptions.len());
        for description in descriptions {
            let item = self
                .items
                .persist(InspectionModelItem {
                    id: None,
                    uuid: None,
                    tenant_id: saved.tenant_id,
                    inspection_model_id: saved.id.unwrap_or_default(),
                    description,
                    created_at: None,
                    created_by: None,
                    updated_at: None,
                    updated_by: None,
                })
                .await
                .map_err(database_error)?;
            items.push(InspectionModelItemEntityMapper::from_active_model(item));
        }
        Ok((saved, items))
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<(InspectionModel, Vec<InspectionModelItem>), BusinessError> {
        let model = self
            .models
            .find_by_uuid(uuid)
            .await
            .map_err(database_error)?
            .map(InspectionModelEntityMapper::from_model)
            .ok_or_else(|| BusinessError::new(MODEL_NOT_FOUND.to_string()))?;
        let items = self.items_of(&model).await?;
        Ok((model, items))
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<(InspectionModel, Vec<InspectionModelItem>)>, u64), BusinessError> {
        let (rows, total) = self.models.find_page(page, page_size).await.map_err(database_error)?;
        let mut models = Vec::with_capacity(rows.len());
        for model in InspectionModelEntityMapper::from_models(rows) {
            let items = self.items_of(&model).await?;
            models.push((model, items));
        }
        Ok((models, total))
    }

    async fn items_of(&self, model: &InspectionModel) -> Result<Vec<InspectionModelItem>, BusinessError> {
        let rows = self.items.find_by_model(model.id.unwrap_or_default()).await.map_err(database_error)?;
        Ok(InspectionModelItemEntityMapper::from_models(rows))
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[InspectionModelUseCase] {}", msg);
    BusinessError::new(msg)
}
