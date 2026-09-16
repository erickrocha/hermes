use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::city::{City, CityEntityMapper};
use crate::gateway::city_gateway::CityGateway;

pub struct CityUseCase {
    gateway: CityGateway,
}

impl CityUseCase {
    pub fn new(gateway: CityGateway) -> Self {
        Self { gateway }
    }

    pub async fn find_by_id(&self, id: i64) -> Result<City, BusinessError> {
        log::info!("[CityUseCase::find_by_id] Executing for id: {}", id);
        let model = self
            .gateway
            .find_by_id(id)
            .await
            .map_err(|e| {
                let msg = format!("Database error: {}", e);
                log::error!("[CityUseCase::find_by_id] {}", msg);
                BusinessError::new(msg)
            })?;

        match model {
            Some(val) => Ok(CityEntityMapper::from_model(val)),
            None => {
                let msg = format!("City not found with id: {}", id);
                log::error!("[CityUseCase::find_by_id] {}", msg);
                Err(BusinessError::new("City not found".to_string()))
            }
        }
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<City, BusinessError> {
        log::info!("[CityUseCase::find_by_uuid] Executing for uuid: {}", uuid);
        let model = self
            .gateway
            .find_by_uuid(uuid.clone())
            .await
            .map_err(|e| {
                let msg = format!("Database error: {}", e);
                log::error!("[CityUseCase::find_by_uuid] {}", msg);
                BusinessError::new(msg)
            })?;

        match model {
            Some(val) => Ok(CityEntityMapper::from_model(val)),
            None => {
                let msg = format!("City not found with uuid: {}", uuid);
                log::error!("[CityUseCase::find_by_uuid] {}", msg);
                Err(BusinessError::new("City not found".to_string()))
            }
        }
    }

    pub async fn find_all(&self) -> Result<Vec<City>, BusinessError> {
        log::info!("[CityUseCase::find_all] Executing find_all");
        let models = self
            .gateway
            .find_all()
            .await
            .map_err(|e| {
                let msg = format!("Database error: {}", e);
                log::error!("[CityUseCase::find_all] {}", msg);
                BusinessError::new(msg)
            })?;

        Ok(CityEntityMapper::from_models(models))
    }

    pub async fn find_by_province_id(&self, province_id: i32) -> Result<Vec<City>, BusinessError> {
        log::info!("[CityUseCase::find_by_province_id] Executing for province_id: {}", province_id);
        let models = self
            .gateway
            .find_by_province_id(province_id)
            .await
            .map_err(|e| {
                let msg = format!("Database error: {}", e);
                log::error!("[CityUseCase::find_by_province_id] {}", msg);
                BusinessError::new(msg)
            })?;

        Ok(CityEntityMapper::from_models(models))
    }
}
