use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::customer::Customer;
use crate::domain::customer_day_off::{CustomerDayOff, CustomerDayOffEntityMapper};
use crate::gateway::customer_day_off_gateway::CustomerDayOffGateway;
use sea_orm::{DbErr, SqlErr};

/// `HRMS-601`: this customer already has a day off recorded for that date
/// (`uq_customer_day_off_customer_date`). Answered 409.
pub const DUPLICATE_DAY_OFF: &str = "This customer already has a day off recorded for that date";
/// The day off has no row with that uuid, or it belongs to another customer.
pub const DAY_OFF_NOT_FOUND: &str = "Day off not found";

pub struct CustomerDayOffUseCase {
    gateway: CustomerDayOffGateway,
}

impl CustomerDayOffUseCase {
    pub fn new(gateway: CustomerDayOffGateway) -> Self {
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
        let day_off = CustomerDayOff {
            id: None,
            uuid: None,
            tenant_id: customer.tenant_id,
            customer_id: customer.id.unwrap_or_default(),
            date,
            reason,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        };
        let saved = self.gateway.persist(day_off).await.map_err(|e| {
            if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
                BusinessError::new(DUPLICATE_DAY_OFF.to_string())
            } else {
                database_error(e)
            }
        })?;
        Ok(CustomerDayOffEntityMapper::from_active_model(saved))
    }

    pub async fn remove(&self, customer: &Customer, uuid: String) -> Result<(), BusinessError> {
        let existing = self.find_visible(customer, uuid).await?;
        self.gateway
            .delete_by_id(existing.id.unwrap_or_default())
            .await
            .map_err(database_error)?;
        Ok(())
    }

    pub async fn history(
        &self,
        customer: &Customer,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<CustomerDayOff>, u64), BusinessError> {
        let (rows, total) = self
            .gateway
            .find_page_by_customer(customer.id.unwrap_or_default(), page, page_size)
            .await
            .map_err(database_error)?;
        Ok((CustomerDayOffEntityMapper::from_models(rows), total))
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
        let model = self
            .gateway
            .find_by_uuid(uuid)
            .await
            .map_err(database_error)?
            .ok_or_else(|| BusinessError::new(DAY_OFF_NOT_FOUND.to_string()))?;
        if model.customer_id != customer.id.unwrap_or_default() {
            return Err(BusinessError::new(DAY_OFF_NOT_FOUND.to_string()));
        }
        Ok(CustomerDayOffEntityMapper::from_model(model))
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[CustomerDayOffUseCase] {}", msg);
    BusinessError::new(msg)
}
