use crate::domain::business_error::BusinessError;
use crate::domain::customer::Customer;
use crate::domain::customer_day_off::CustomerDayOff;
use crate::gateway::customer_day_off_gateway::{CustomerDayOffStore, CustomerDayOffStoreError};

/// `HRMS-601`: this customer already has a day off recorded for that date
/// (`uq_customer_day_off_customer_date`). Answered 409.
pub const DUPLICATE_DAY_OFF: &str = "This customer already has a day off recorded for that date";
/// The day off has no row with that uuid, or it belongs to another customer.
pub const DAY_OFF_NOT_FOUND: &str = "Day off not found";

pub struct CustomerDayOffUseCase<G: CustomerDayOffStore> {
    gateway: G,
}

impl<G: CustomerDayOffStore> CustomerDayOffUseCase<G> {
    pub fn new(gateway: G) -> Self {
        Self { gateway }
    }

    /// `EPIC-SC-01-S02`. The caller has already been allowed to administer
    /// `customer` (`can_administer_customer`, checked at the endpoint).
    pub async fn add(
        &self,
        customer: &Customer,
        date: sea_orm::prelude::Date,
        reason: Option<String>,
    ) -> Result<CustomerDayOff, BusinessError> {
        let customer_id = persisted_customer_id(customer)?;
        let day_off = CustomerDayOff {
            id: None,
            uuid: None,
            tenant_id: customer.tenant_id,
            customer_id,
            date,
            reason,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        };
        self.gateway
            .save(day_off)
            .await
            .map_err(|error| match error {
                CustomerDayOffStoreError::DuplicateDate => {
                    BusinessError::new(DUPLICATE_DAY_OFF.to_string())
                }
                CustomerDayOffStoreError::Storage(error) => error,
            })
    }

    pub async fn remove(&self, customer: &Customer, uuid: String) -> Result<(), BusinessError> {
        let existing = self.find_visible(customer, uuid).await?;
        let id = existing
            .id
            .filter(|id| *id > 0)
            .ok_or_else(|| BusinessError::new(DAY_OFF_NOT_FOUND.to_string()))?;
        self.gateway.delete_by_id(id).await?;
        Ok(())
    }

    pub async fn history(
        &self,
        customer: &Customer,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<CustomerDayOff>, u64), BusinessError> {
        let customer_id = persisted_customer_id(customer)?;
        self.gateway
            .list_page_by_customer(customer_id, page, page_size)
            .await
    }

    /// The uuid must resolve to a row belonging to *this* customer -- not
    /// merely to the caller's tenant -- so deleting `/customer/A/day-off/{id
    /// of a day-off actually on customer B}` is refused rather than
    /// silently removing the wrong customer's row.
    async fn find_visible(
        &self,
        customer: &Customer,
        uuid: String,
    ) -> Result<CustomerDayOff, BusinessError> {
        let customer_id = persisted_customer_id(customer)?;
        let day_off = self
            .gateway
            .get_by_uuid(uuid)
            .await?
            .ok_or_else(|| BusinessError::new(DAY_OFF_NOT_FOUND.to_string()))?;
        if day_off.customer_id != customer_id {
            return Err(BusinessError::new(DAY_OFF_NOT_FOUND.to_string()));
        }
        Ok(day_off)
    }
}

fn persisted_customer_id(customer: &Customer) -> Result<i64, BusinessError> {
    customer
        .id
        .filter(|id| *id > 0)
        .ok_or_else(|| BusinessError::new("Customer identity is incomplete".to_string()))
}
