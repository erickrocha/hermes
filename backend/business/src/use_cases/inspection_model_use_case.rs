use crate::domain::business_error::BusinessError;
use crate::domain::inspection_model::InspectionModel;
use crate::domain::inspection_model_item::InspectionModelItem;
use crate::gateway::inspection_model_gateway::InspectionModelStore;
use crate::gateway::inspection_model_item_gateway::InspectionModelItemStore;

pub const NAME_REQUIRED: &str = "An inspection model needs a name";
pub const ITEMS_REQUIRED: &str = "An inspection model needs at least one non-blank item";
pub const PERIODICITY_INVALID: &str = "The periodicity must be a positive number of days";
pub const DUPLICATE_NAME: &str = "This tenant already has an inspection model with this name";
pub const MODEL_NOT_FOUND: &str = "Inspection model not found";

/// `EPIC-MT-07-S10` (`HRMS-715`): technical-inspection templates.
pub struct InspectionModelUseCase {
    models: Box<dyn InspectionModelStore>,
    items: Box<dyn InspectionModelItemStore>,
}

impl InspectionModelUseCase {
    pub fn new<M, I>(models: M, items: I) -> Self
    where
        M: InspectionModelStore + 'static,
        I: InspectionModelItemStore + 'static,
    {
        Self {
            models: Box::new(models),
            items: Box::new(items),
        }
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
        let descriptions: Vec<String> = descriptions
            .into_iter()
            .map(|d| d.trim().to_string())
            .filter(|d| !d.is_empty())
            .collect();
        if descriptions.is_empty() {
            return Err(BusinessError::new(ITEMS_REQUIRED.to_string()));
        }
        if model.periodicity_days.is_some_and(|d| d <= 0) {
            return Err(BusinessError::new(PERIODICITY_INVALID.to_string()));
        }
        if self
            .models
            .find_by_name(&name, model.tenant_id)
            .await?
            .is_some()
        {
            return Err(BusinessError::new(DUPLICATE_NAME.to_string()));
        }

        let saved = self.models.save(InspectionModel { name, ..model }).await?;
        let model_id = saved.id.filter(|id| *id > 0).ok_or_else(|| {
            BusinessError::new("Inspection model identity is incomplete".to_string())
        })?;
        let mut items = Vec::with_capacity(descriptions.len());
        for description in descriptions {
            let item = self
                .items
                .save(InspectionModelItem {
                    id: None,
                    uuid: None,
                    tenant_id: saved.tenant_id,
                    inspection_model_id: model_id,
                    description,
                    created_at: None,
                    created_by: None,
                    updated_at: None,
                    updated_by: None,
                })
                .await?;
            items.push(item);
        }
        Ok((saved, items))
    }

    pub async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<(InspectionModel, Vec<InspectionModelItem>), BusinessError> {
        let model = self
            .models
            .get_by_uuid(uuid)
            .await?
            .ok_or_else(|| BusinessError::new(MODEL_NOT_FOUND.to_string()))?;
        let items = self.items_of(&model).await?;
        Ok((model, items))
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<(InspectionModel, Vec<InspectionModelItem>)>, u64), BusinessError> {
        let (rows, total) = self.models.list_page(page, page_size).await?;
        let mut models = Vec::with_capacity(rows.len());
        for model in rows {
            let items = self.items_of(&model).await?;
            models.push((model, items));
        }
        Ok((models, total))
    }

    async fn items_of(
        &self,
        model: &InspectionModel,
    ) -> Result<Vec<InspectionModelItem>, BusinessError> {
        let model_id = model.id.filter(|id| *id > 0).ok_or_else(|| {
            BusinessError::new("Inspection model identity is incomplete".to_string())
        })?;
        self.items.list_by_model(model_id).await
    }
}
