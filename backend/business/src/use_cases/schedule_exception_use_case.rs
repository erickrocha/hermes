use crate::domain::business_error::BusinessError;
use crate::domain::authorization::is_active_driver_of;
use crate::domain::schedule_exception::ScheduleException;
use crate::gateway::schedule_exception_gateway::ScheduleExceptionStore;
use crate::gateway::user_gateway::UserLookupStore;
use crate::gateway::vehicle_gateway::VehicleLookupStore;

/// `HRMS-606`: `new_driver_id` (when named) does not name an active
/// `Driver` of the exception's own tenant.
pub const NOT_A_DRIVER: &str = "The person named is not an active driver of this tenant";
/// `new_vehicle_id` (when named) does not name a vehicle of the exception's
/// own tenant.
pub const NOT_A_TENANT_VEHICLE: &str = "The vehicle named does not belong to this tenant";

pub struct ScheduleExceptionUseCase {
    gateway: Box<dyn ScheduleExceptionStore>,
    users: Box<dyn UserLookupStore>,
    vehicles: Box<dyn VehicleLookupStore>,
}

impl ScheduleExceptionUseCase {
    pub fn new<G, U, V>(gateway: G, users: U, vehicles: V) -> Self
    where
        G: ScheduleExceptionStore + 'static,
        U: UserLookupStore + 'static,
        V: VehicleLookupStore + 'static,
    {
        Self {
            gateway: Box::new(gateway),
            users: Box::new(users),
            vehicles: Box::new(vehicles),
        }
    }

    /// `EPIC-SC-02-S04`. The caller has already been allowed to administer
    /// the demand this exception belongs to (checked at the endpoint).
    pub async fn create(
        &self,
        exception: ScheduleException,
    ) -> Result<ScheduleException, BusinessError> {
        let exception = self.validated(exception).await?;
        self.gateway.save(exception).await
    }

    /// Both replacement fields are optional (a cancellation names neither);
    /// each is validated only when present, the same shape
    /// `transport_demand_use_case::validated` treats its own optional
    /// preferences.
    async fn validated(
        &self,
        exception: ScheduleException,
    ) -> Result<ScheduleException, BusinessError> {
        if let Some(driver_id) = exception.new_driver_id {
            let driver = self.users.get_by_id(driver_id).await?;
            let is_valid_driver = driver.is_some_and(|user| {
                is_active_driver_of(&user, exception.tenant_id)
            });
            if !is_valid_driver {
                return Err(BusinessError::new(NOT_A_DRIVER.to_string()));
            }
        }

        if let Some(vehicle_id) = exception.new_vehicle_id {
            let vehicle = self.vehicles.get_by_id(vehicle_id).await?;
            let belongs_to_tenant =
                vehicle.is_some_and(|vehicle| vehicle.tenant_id == exception.tenant_id);
            if !belongs_to_tenant {
                return Err(BusinessError::new(NOT_A_TENANT_VEHICLE.to_string()));
            }
        }

        Ok(exception)
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<ScheduleException, BusinessError> {
        self.gateway
            .get_by_uuid(uuid)
            .await?
            .ok_or_else(|| BusinessError::new("Exception not found".to_string()))
    }

    pub async fn remove(&self, demand_id: i64, uuid: String) -> Result<(), BusinessError> {
        let existing = self.find_by_uuid(uuid).await?;
        if existing.demand_id != demand_id {
            return Err(BusinessError::new("Exception not found".to_string()));
        }
        let id = existing.id.filter(|id| *id > 0).ok_or_else(|| {
            BusinessError::new("Schedule exception identity is incomplete".to_string())
        })?;
        self.gateway.delete_by_id(id).await?;
        Ok(())
    }

    pub async fn history(
        &self,
        demand_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<ScheduleException>, u64), BusinessError> {
        self.gateway
            .list_by_demand(demand_id, page, page_size)
            .await
    }
}
