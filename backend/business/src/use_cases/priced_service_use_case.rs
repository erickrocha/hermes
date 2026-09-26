use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::priced_service::{PricedService, PricedServiceEntityMapper};
use crate::gateway::priced_service_gateway::PricedServiceGateway;
use sea_orm::DbErr;

pub const NAME_REQUIRED: &str = "A priced service needs a name";

pub struct PricedServiceUseCase {
    gateway: PricedServiceGateway,
}

impl PricedServiceUseCase {
    pub fn new(gateway: PricedServiceGateway) -> Self {
        Self { gateway }
    }

    pub async fn create(&self, priced_service: PricedService) -> Result<PricedService, BusinessError> {
        let name = priced_service.name.trim().to_string();
        if name.is_empty() {
            return Err(BusinessError::new(NAME_REQUIRED.to_string()));
        }
        let entity = self
            .gateway
            .persist(PricedService { name, ..priced_service })
            .await
            .map_err(database_error)?;
        Ok(PricedServiceEntityMapper::from_active_model(entity))
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<PricedService, BusinessError> {
        let entity = self.gateway.find_by_uuid(uuid).await.map_err(database_error)?;
        match entity {
            Some(model) => Ok(PricedServiceEntityMapper::from_model(model)),
            None => Err(BusinessError::new("Priced service not found".to_string())),
        }
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<PricedService>, u64), BusinessError> {
        let (rows, total) = self.gateway.find_page(page, page_size).await.map_err(database_error)?;
        Ok((PricedServiceEntityMapper::from_models(rows), total))
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[PricedServiceUseCase] {}", msg);
    BusinessError::new(msg)
}
