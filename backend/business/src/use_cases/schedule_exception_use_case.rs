use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::Role;
use crate::domain::schedule_exception::{ScheduleException, ScheduleExceptionEntityMapper};
use crate::gateway::schedule_exception_gateway::ScheduleExceptionGateway;
use crate::gateway::user_gateway::UserGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use sea_orm::DbErr;

/// `HRMS-606`: `new_driver_id` (when named) does not name an active
/// `Driver` of the exception's own tenant.
pub const NOT_A_DRIVER: &str = "The person named is not an active driver of this tenant";
/// `new_vehicle_id` (when named) does not name a vehicle of the exception's
/// own tenant.
pub const NOT_A_TENANT_VEHICLE: &str = "The vehicle named does not belong to this tenant";

pub struct ScheduleExceptionUseCase {
    gateway: ScheduleExceptionGateway,
    users: UserGateway,
    vehicles: VehicleGateway,
}

impl ScheduleExceptionUseCase {
    pub fn new(gateway: ScheduleExceptionGateway, users: UserGateway, vehicles: VehicleGateway) -> Self {
        Self {
            gateway,
            users,
            vehicles,
        }
    }

    /// `EPIC-SC-02-S04`. The caller has already been allowed to administer
    /// the demand this exception belongs to (checked at the endpoint).
    pub async fn create(&self, exception: ScheduleException) -> Result<ScheduleException, BusinessError> {
        let exception = self.validated(exception).await?;
        let entity = self
            .gateway
            .persist(exception)
            .await
            .map_err(database_error)?;
        Ok(ScheduleExceptionEntityMapper::from_active_model(entity))
    }

    /// Both replacement fields are optional (a cancellation names neither);
    /// each is validated only when present, the same shape
    /// `transport_demand_use_case::validated` treats its own optional
    /// preferences.
    async fn validated(&self, exception: ScheduleException) -> Result<ScheduleException, BusinessError> {
        if let Some(driver_id) = exception.new_driver_id {
            let driver = self
                .users
                .find_by_id(driver_id)
                .await
                .map_err(database_error)?;
            let is_valid_driver = driver.is_some_and(|user| {
                user.role == Role::Driver.to_string()
                    && user.enabled
                    && user.tenant_id == exception.tenant_id
            });
            if !is_valid_driver {
                return Err(BusinessError::new(NOT_A_DRIVER.to_string()));
            }
        }

        if let Some(vehicle_id) = exception.new_vehicle_id {
            let vehicle = self
                .vehicles
                .find_by_id(vehicle_id)
                .await
                .map_err(database_error)?;
            let belongs_to_tenant = vehicle.is_some_and(|v| v.tenant_id == exception.tenant_id);
            if !belongs_to_tenant {
                return Err(BusinessError::new(NOT_A_TENANT_VEHICLE.to_string()));
            }
        }

        Ok(exception)
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<ScheduleException, BusinessError> {
        let entity = self
            .gateway
            .find_by_uuid(uuid)
            .await
            .map_err(database_error)?;
        match entity {
            Some(model) => Ok(ScheduleExceptionEntityMapper::from_model(model)),
            None => Err(BusinessError::new("Exception not found".to_string())),
        }
    }

    pub async fn remove(&self, demand_id: i64, uuid: String) -> Result<(), BusinessError> {
        let existing = self.find_by_uuid(uuid).await?;
        if existing.demand_id != demand_id {
            return Err(BusinessError::new("Exception not found".to_string()));
        }
        self.gateway
            .delete_by_id(existing.id.unwrap_or_default())
            .await
            .map_err(database_error)?;
        Ok(())
    }

    pub async fn history(
        &self,
        demand_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<ScheduleException>, u64), BusinessError> {
        let (rows, total) = self
            .gateway
            .find_page_by_demand(demand_id, page, page_size)
            .await
            .map_err(database_error)?;
        Ok((ScheduleExceptionEntityMapper::from_models(rows), total))
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[ScheduleExceptionUseCase] {}", msg);
    BusinessError::new(msg)
}
