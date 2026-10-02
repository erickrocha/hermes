use crate::domain::business_error::BusinessError;
use crate::domain::priced_service::PricedService;
use crate::gateway::priced_service_gateway::PricedServiceStore;

pub const NAME_REQUIRED: &str = "A priced service needs a name";

pub struct PricedServiceUseCase<G: PricedServiceStore> {
    gateway: G,
}

impl<G: PricedServiceStore> PricedServiceUseCase<G> {
    pub fn new(gateway: G) -> Self {
        Self { gateway }
    }

    pub async fn create(
        &self,
        priced_service: PricedService,
    ) -> Result<PricedService, BusinessError> {
        let name = priced_service.name.trim().to_string();
        if name.is_empty() {
            return Err(BusinessError::new(NAME_REQUIRED.to_string()));
        }
        self.gateway
            .save(PricedService {
                name,
                ..priced_service
            })
            .await
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<PricedService, BusinessError> {
        match self.gateway.get_by_uuid(uuid).await? {
            Some(priced_service) => Ok(priced_service),
            None => Err(BusinessError::new("Priced service not found".to_string())),
        }
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<PricedService>, u64), BusinessError> {
        self.gateway.list_page(page, page_size).await
    }
}
