use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::Role;
use crate::domain::transport_demand_allocation::{
    TransportDemandAllocation, TransportDemandAllocationEntityMapper,
};
use crate::gateway::transport_demand_allocation_gateway::TransportDemandAllocationGateway;
use crate::gateway::user_gateway::UserGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use sea_orm::DbErr;

/// `HRMS-604`: `driver_id` does not name an active `Driver` of the
/// allocation's own tenant. Same shape `transport_demand_use_case`/
/// `vehicle_assignment_use_case` use.
pub const NOT_A_DRIVER: &str = "The person named is not an active driver of this tenant";
/// `vehicle_id` does not name a vehicle of the allocation's own tenant.
pub const NOT_A_TENANT_VEHICLE: &str = "The vehicle named does not belong to this tenant";

pub struct TransportDemandAllocationUseCase {
    gateway: TransportDemandAllocationGateway,
    users: UserGateway,
    vehicles: VehicleGateway,
}

impl TransportDemandAllocationUseCase {
    pub fn new(
        gateway: TransportDemandAllocationGateway,
        users: UserGateway,
        vehicles: VehicleGateway,
    ) -> Self {
        Self {
            gateway,
            users,
            vehicles,
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
        let entity = self
            .gateway
            .persist(allocation)
            .await
            .map_err(database_error)?;
        Ok(TransportDemandAllocationEntityMapper::from_active_model(entity))
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

        let entity = self
            .gateway
            .persist(updated)
            .await
            .map_err(database_error)?;
        Ok(TransportDemandAllocationEntityMapper::from_active_model(entity))
    }

    /// `driver_id` must be an active `Driver` of the allocation's own
    /// tenant; `vehicle_id` must belong to it too -- both required here,
    /// unlike the demand's own optional preferences.
    async fn validated(
        &self,
        allocation: TransportDemandAllocation,
    ) -> Result<TransportDemandAllocation, BusinessError> {
        let driver = self
            .users
            .find_by_id(allocation.driver_id)
            .await
            .map_err(database_error)?;
        let is_valid_driver = driver.is_some_and(|user| {
            user.role == Role::Driver.to_string()
                && user.enabled
                && user.tenant_id == allocation.tenant_id
        });
        if !is_valid_driver {
            return Err(BusinessError::new(NOT_A_DRIVER.to_string()));
        }

        let vehicle = self
            .vehicles
            .find_by_id(allocation.vehicle_id)
            .await
            .map_err(database_error)?;
        let belongs_to_tenant = vehicle.is_some_and(|v| v.tenant_id == allocation.tenant_id);
        if !belongs_to_tenant {
            return Err(BusinessError::new(NOT_A_TENANT_VEHICLE.to_string()));
        }

        Ok(allocation)
    }

    pub async fn find_by_id(&self, id: i64) -> Result<TransportDemandAllocation, BusinessError> {
        let entity = self.gateway.find_by_id(id).await.map_err(database_error)?;
        match entity {
            Some(model) => Ok(TransportDemandAllocationEntityMapper::from_model(model)),
            None => Err(BusinessError::new("Allocation not found".to_string())),
        }
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<TransportDemandAllocation, BusinessError> {
        let entity = self
            .gateway
            .find_by_uuid(uuid)
            .await
            .map_err(database_error)?;
        match entity {
            Some(model) => Ok(TransportDemandAllocationEntityMapper::from_model(model)),
            None => Err(BusinessError::new("Allocation not found".to_string())),
        }
    }

    pub async fn history(
        &self,
        demand_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<TransportDemandAllocation>, u64), BusinessError> {
        let (rows, total) = self
            .gateway
            .find_page_by_demand(demand_id, page, page_size)
            .await
            .map_err(database_error)?;
        Ok((TransportDemandAllocationEntityMapper::from_models(rows), total))
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[TransportDemandAllocationUseCase] {}", msg);
    BusinessError::new(msg)
}
