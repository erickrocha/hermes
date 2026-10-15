use crate::domain::business_error::BusinessError;
use crate::domain::service_type::ServiceType;
use crate::gateway::service_type_gateway::ServiceTypeStore;

pub const CODE_REQUIRED: &str = "A service type needs a code";
pub const NAME_REQUIRED: &str = "A service type needs a name";
pub const DUPLICATE_CODE: &str = "This tenant already has a service type with this code";

pub struct ServiceTypeUseCase<G: ServiceTypeStore> {
    gateway: G,
}

impl<G: ServiceTypeStore> ServiceTypeUseCase<G> {
    pub fn new(gateway: G) -> Self {
        Self { gateway }
    }

    pub async fn create(&self, service_type: ServiceType) -> Result<ServiceType, BusinessError> {
        let service_type = self.validated(service_type).await?;
        self.gateway.save(service_type).await
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
            .get_by_code(&code, service_type.tenant_id)
            .await?;
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
        match self.gateway.get_by_uuid(uuid).await? {
            Some(service_type) => Ok(service_type),
            None => Err(BusinessError::new("Service type not found".to_string())),
        }
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<ServiceType>, u64), BusinessError> {
        self.gateway.list_page(page, page_size).await
    }
}
