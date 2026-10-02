use crate::domain::business_error::BusinessError;
use crate::domain::daily_schedule::DailySchedule;
use crate::domain::enums::Role;
use crate::gateway::daily_schedule_gateway::{DailyScheduleStore, DailyScheduleStoreError};
use crate::gateway::user_gateway::UserLookupStore;
use crate::gateway::vehicle_gateway::VehicleLookupStore;

/// `HRMS-605`: `driver_id` does not name an active `Driver` of the entry's
/// own tenant. Same shape every other allocation-style use case here uses.
pub const NOT_A_DRIVER: &str = "The person named is not an active driver of this tenant";
/// `vehicle_id` does not name a vehicle of the entry's own tenant.
pub const NOT_A_TENANT_VEHICLE: &str = "The vehicle named does not belong to this tenant";
/// This demand already has a manual entry for that date
/// (`uq_daily_schedule_demand_date`).
pub const DUPLICATE_SCHEDULE_ENTRY: &str = "This demand already has a schedule entry for that date";

pub struct DailyScheduleUseCase {
    gateway: Box<dyn DailyScheduleStore>,
    users: Box<dyn UserLookupStore>,
    vehicles: Box<dyn VehicleLookupStore>,
}

impl DailyScheduleUseCase {
    pub fn new<S, U, V>(gateway: S, users: U, vehicles: V) -> Self
    where
        S: DailyScheduleStore + 'static,
        U: UserLookupStore + 'static,
        V: VehicleLookupStore + 'static,
    {
        Self {
            gateway: Box::new(gateway),
            users: Box::new(users),
            vehicles: Box::new(vehicles),
        }
    }

    /// `EPIC-SC-02-S03`. The caller has already been allowed to administer
    /// the demand this entry belongs to (checked at the endpoint).
    pub async fn create(&self, entry: DailySchedule) -> Result<DailySchedule, BusinessError> {
        let entry = self.validated(entry).await?;
        self.gateway.save(entry).await.map_err(|error| match error {
            DailyScheduleStoreError::Duplicate => {
                BusinessError::new(DUPLICATE_SCHEDULE_ENTRY.to_string())
            }
            DailyScheduleStoreError::Storage(error) => error,
        })
    }

    /// `driver_id` must be an active `Driver` of the entry's own tenant;
    /// `vehicle_id` must belong to it too -- same checks
    /// `transport_demand_allocation_use_case::validated` runs.
    async fn validated(&self, entry: DailySchedule) -> Result<DailySchedule, BusinessError> {
        let driver = self.users.get_by_id(entry.driver_id).await?;
        let is_valid_driver = driver.is_some_and(|user| {
            user.role == Role::Driver && user.enabled && user.tenant_id == entry.tenant_id
        });
        if !is_valid_driver {
            return Err(BusinessError::new(NOT_A_DRIVER.to_string()));
        }

        let vehicle = self.vehicles.get_by_id(entry.vehicle_id).await?;
        let belongs_to_tenant = vehicle.is_some_and(|v| v.tenant_id == entry.tenant_id);
        if !belongs_to_tenant {
            return Err(BusinessError::new(NOT_A_TENANT_VEHICLE.to_string()));
        }

        Ok(entry)
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<DailySchedule, BusinessError> {
        let entity = self.gateway.get_by_uuid(uuid).await?;
        match entity {
            Some(entry) => Ok(entry),
            None => Err(BusinessError::new("Schedule entry not found".to_string())),
        }
    }

    pub async fn remove(&self, demand_id: i64, uuid: String) -> Result<(), BusinessError> {
        let existing = self.find_by_uuid(uuid).await?;
        if existing.demand_id != demand_id {
            return Err(BusinessError::new("Schedule entry not found".to_string()));
        }
        self.gateway
            .delete_by_id(existing.id.unwrap_or_default())
            .await?;
        Ok(())
    }

    pub async fn history(
        &self,
        demand_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<DailySchedule>, u64), BusinessError> {
        self.gateway
            .list_page_by_demand(demand_id, page, page_size)
            .await
    }
}
