use crate::domain::business_error::BusinessError;
use crate::domain::enums::Role;
use crate::domain::transport_demand_allocation::TransportDemandAllocation;
use crate::gateway::transport_demand_allocation_gateway::TransportDemandAllocationStore;
use crate::gateway::user_gateway::UserLookupStore;
use crate::gateway::vehicle_gateway::VehicleLookupStore;

/// `HRMS-604`: `driver_id` does not name an active `Driver` of the
/// allocation's own tenant. Same shape `transport_demand_use_case`/
/// `vehicle_assignment_use_case` use.
pub const NOT_A_DRIVER: &str = "The person named is not an active driver of this tenant";
/// `vehicle_id` does not name a vehicle of the allocation's own tenant.
/// `TRM-003`: an allocation's own days of the week, when given, must be readable.
pub const DAYS_OF_WEEK_UNREADABLE: &str =
    "The days of the week must be a list of weekdays (Mon, Tue, ... or 1-7)";
pub const NOT_A_TENANT_VEHICLE: &str = "The vehicle named does not belong to this tenant";
/// `HRMS-604`: an allocation covers a range of dates; one that ends before it starts covers none.
pub const ENDS_BEFORE_IT_STARTS: &str = "The allocation ends before it starts";

pub struct TransportDemandAllocationUseCase {
    gateway: Box<dyn TransportDemandAllocationStore>,
    users: Box<dyn UserLookupStore>,
    vehicles: Box<dyn VehicleLookupStore>,
}

impl TransportDemandAllocationUseCase {
    pub fn new<G, U, V>(gateway: G, users: U, vehicles: V) -> Self
    where
        G: TransportDemandAllocationStore + 'static,
        U: UserLookupStore + 'static,
        V: VehicleLookupStore + 'static,
    {
        Self {
            gateway: Box::new(gateway),
            users: Box::new(users),
            vehicles: Box::new(vehicles),
        }
    }

    /// `EPIC-SC-02-S02`. The caller has already been allowed to administer
    /// the demand this allocation belongs to (checked at the endpoint, the
    /// same shape `vehicle_assignment_endpoint` uses for its own vehicle).
    pub async fn create(
        &self,
        allocation: TransportDemandAllocation,
    ) -> Result<TransportDemandAllocation, BusinessError> {
        let allocation = self.validated(allocation).await?;
        self.gateway.save(allocation).await
    }

    pub async fn update(
        &self,
        id: i64,
        allocation: TransportDemandAllocation,
    ) -> Result<TransportDemandAllocation, BusinessError> {
        let existing = self.find_by_id(id).await?;
        let allocation = self.validated(allocation).await?;

        let updated = TransportDemandAllocation {
            id: Some(id),
            uuid: existing.uuid,
            tenant_id: existing.tenant_id,
            demand_id: existing.demand_id,
            created_at: existing.created_at,
            created_by: existing.created_by,
            updated_at: None,
            ..allocation
        };

        self.gateway.save(updated).await
    }

    /// `driver_id` must be an active `Driver` of the allocation's own
    /// tenant; `vehicle_id` must belong to it too -- both required here,
    /// unlike the demand's own optional preferences.
    async fn validated(
        &self,
        allocation: TransportDemandAllocation,
    ) -> Result<TransportDemandAllocation, BusinessError> {
        if allocation.days_of_week.as_deref().is_some_and(|d| {
            !d.trim().is_empty()
                && crate::use_cases::effective_schedule::parse_days_of_week(d).is_none()
        }) {
            return Err(BusinessError::new(DAYS_OF_WEEK_UNREADABLE.to_string()));
        }
        if allocation
            .end_date
            .is_some_and(|end| end < allocation.start_date)
        {
            return Err(BusinessError::new(ENDS_BEFORE_IT_STARTS.to_string()));
        }
        let driver = self.users.get_by_id(allocation.driver_id).await?;
        let is_valid_driver = driver.is_some_and(|user| {
            user.role == Role::Driver && user.enabled && user.tenant_id == allocation.tenant_id
        });
        if !is_valid_driver {
            return Err(BusinessError::new(NOT_A_DRIVER.to_string()));
        }

        let vehicle = self.vehicles.get_by_id(allocation.vehicle_id).await?;
        let belongs_to_tenant =
            vehicle.is_some_and(|vehicle| vehicle.tenant_id == allocation.tenant_id);
        if !belongs_to_tenant {
            return Err(BusinessError::new(NOT_A_TENANT_VEHICLE.to_string()));
        }

        Ok(allocation)
    }

    pub async fn find_by_id(&self, id: i64) -> Result<TransportDemandAllocation, BusinessError> {
        self.gateway
            .get_by_id(id)
            .await?
            .ok_or_else(|| BusinessError::new("Allocation not found".to_string()))
    }

    pub async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<TransportDemandAllocation, BusinessError> {
        self.gateway
            .get_by_uuid(uuid)
            .await?
            .ok_or_else(|| BusinessError::new("Allocation not found".to_string()))
    }

    pub async fn history(
        &self,
        demand_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<TransportDemandAllocation>, u64), BusinessError> {
        self.gateway
            .list_by_demand(demand_id, page, page_size)
            .await
    }
}
