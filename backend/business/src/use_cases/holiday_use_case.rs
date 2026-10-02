use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::holiday::{Holiday, HolidayEntityMapper};
use crate::gateway::holiday_gateway::HolidayGateway;
use sea_orm::{DbErr, SqlErr};

/// `HRMS-602`: this tenant already has a holiday recorded for that date
/// (`uq_holiday_tenant_date`). Answered 409.
pub const DUPLICATE_HOLIDAY: &str = "A holiday is already recorded for that date";

pub struct HolidayUseCase {
    gateway: HolidayGateway,
}

impl HolidayUseCase {
    pub fn new(gateway: HolidayGateway) -> Self {
        Self { gateway }
    }

    pub async fn create(&self, holiday: Holiday) -> Result<Holiday, BusinessError> {
        let holiday = Self::validated(holiday)?;
        let saved = self.gateway.persist(holiday).await.map_err(|e| {
            if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
                BusinessError::new(DUPLICATE_HOLIDAY.to_string())
            } else {
                database_error(e)
            }
        })?;
        Ok(HolidayEntityMapper::from_active_model(saved))
    }

    fn validated(holiday: Holiday) -> Result<Holiday, BusinessError> {
        let name = holiday.name.trim().to_string();
        if name.is_empty() {
            let msg = "Holiday name is required".to_string();
            log::error!("[HolidayUseCase::validated] {}", msg);
            return Err(BusinessError::new(msg));
        }
        Ok(Holiday { name, ..holiday })
    }

    pub async fn find_by_id(&self, id: i64) -> Result<Holiday, BusinessError> {
        let entity = self.gateway.find_by_id(id).await.map_err(database_error)?;
        match entity {
            Some(model) => Ok(HolidayEntityMapper::from_model(model)),
            None => Err(BusinessError::new("Holiday not found".to_string())),
        }
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<Holiday, BusinessError> {
        let entity = self
            .gateway
            .find_by_uuid(uuid)
            .await
            .map_err(database_error)?;
        match entity {
            Some(model) => Ok(HolidayEntityMapper::from_model(model)),
            None => Err(BusinessError::new("Holiday not found".to_string())),
        }
    }

    pub async fn remove(&self, uuid: String) -> Result<(), BusinessError> {
        let existing = self.find_by_uuid(uuid).await?;
        self.gateway
            .delete_by_id(existing.id.unwrap_or_default())
            .await
            .map_err(database_error)?;
        Ok(())
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<Holiday>, u64), BusinessError> {
        let (entities, total) = self
            .gateway
            .find_page(page, page_size)
            .await
            .map_err(database_error)?;
        Ok((HolidayEntityMapper::from_models(entities), total))
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[HolidayUseCase] {}", msg);
    BusinessError::new(msg)
}
