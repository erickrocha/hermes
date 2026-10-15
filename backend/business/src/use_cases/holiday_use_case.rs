use crate::domain::business_error::BusinessError;
use crate::domain::holiday::Holiday;
use crate::gateway::holiday_gateway::{HolidayStore, HolidayStoreError};

/// `HRMS-602`: this tenant already has a holiday recorded for that date
/// (`uq_holiday_tenant_date`). Answered 409.
pub const DUPLICATE_HOLIDAY: &str = "A holiday is already recorded for that date";

pub struct HolidayUseCase<G: HolidayStore> {
    gateway: G,
}

impl<G: HolidayStore> HolidayUseCase<G> {
    pub fn new(gateway: G) -> Self {
        Self { gateway }
    }

    pub async fn create(&self, holiday: Holiday) -> Result<Holiday, BusinessError> {
        let holiday = Self::validated(holiday)?;
        self.gateway
            .save(holiday)
            .await
            .map_err(|error| match error {
                HolidayStoreError::DuplicateDate => {
                    BusinessError::new(DUPLICATE_HOLIDAY.to_string())
                }
                HolidayStoreError::Storage(error) => error,
            })
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
        let holiday = self.gateway.get_by_id(id).await?;
        match holiday {
            Some(holiday) => Ok(holiday),
            None => Err(BusinessError::new("Holiday not found".to_string())),
        }
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<Holiday, BusinessError> {
        let holiday = self.gateway.get_by_uuid(uuid).await?;
        match holiday {
            Some(holiday) => Ok(holiday),
            None => Err(BusinessError::new("Holiday not found".to_string())),
        }
    }

    pub async fn remove(&self, uuid: String) -> Result<(), BusinessError> {
        let existing = self.find_by_uuid(uuid).await?;
        let id = existing
            .id
            .filter(|id| *id > 0)
            .ok_or_else(|| BusinessError::new("Holiday identity is incomplete".to_string()))?;
        self.gateway.delete_by_id(id).await?;
        Ok(())
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<Holiday>, u64), BusinessError> {
        self.gateway.list_page(page, page_size).await
    }
}
