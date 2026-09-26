use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::service_type::{ServiceType, ServiceTypeEntityMapper};
use crate::gateway::service_type_gateway::ServiceTypeGateway;
use sea_orm::DbErr;

pub const CODE_REQUIRED: &str = "A service type needs a code";
pub const NAME_REQUIRED: &str = "A service type needs a name";
pub const DUPLICATE_CODE: &str = "This tenant already has a service type with this code";

pub struct ServiceTypeUseCase {
    gateway: ServiceTypeGateway,
}

impl ServiceTypeUseCase {
    pub fn new(gateway: ServiceTypeGateway) -> Self {
        Self { gateway }
    }

    pub async fn create(&self, service_type: ServiceType) -> Result<ServiceType, BusinessError> {
        let service_type = self.validated(service_type).await?;
        let entity = self.gateway.persist(service_type).await.map_err(database_error)?;
        Ok(ServiceTypeEntityMapper::from_active_model(entity))
    }

    async fn validated(&self, service_type: ServiceType) -> Result<ServiceType, BusinessError> {
        let code = service_type.code.trim().to_string();
        if code.is_empty() {
            return Err(BusinessError::new(CODE_REQUIRED.to_string()));
        }
        let name = service_type.name.trim().to_string();
        if name.is_empty() {
            return Err(BusinessError::new(NAME_REQUIRED.to_string()));
        }

        let existing = self
            .gateway
            .find_by_code(&code, service_type.tenant_id)
            .await
            .map_err(database_error)?;
        if existing.is_some() {
            return Err(BusinessError::new(DUPLICATE_CODE.to_string()));
        }

        Ok(ServiceType {
            code,
            name,
            ..service_type
        })
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<ServiceType, BusinessError> {
        let entity = self.gateway.find_by_uuid(uuid).await.map_err(database_error)?;
        match entity {
            Some(model) => Ok(ServiceTypeEntityMapper::from_model(model)),
            None => Err(BusinessError::new("Service type not found".to_string())),
        }
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<ServiceType>, u64), BusinessError> {
        let (rows, total) = self.gateway.find_page(page, page_size).await.map_err(database_error)?;
        Ok((ServiceTypeEntityMapper::from_models(rows), total))
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[ServiceTypeUseCase] {}", msg);
    BusinessError::new(msg)
}
