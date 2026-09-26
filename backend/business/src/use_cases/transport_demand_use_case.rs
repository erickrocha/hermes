use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::Role;
use crate::domain::transport_demand::{TransportDemand, TransportDemandEntityMapper};
use crate::gateway::transport_demand_gateway::TransportDemandGateway;
use crate::gateway::user_gateway::UserGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use sea_orm::DbErr;

/// `HRMS-603`: `specific_driver_id` does not name an active `Driver` of the
/// demand's own tenant. Same shape `vehicle_assignment_use_case::NOT_A_DRIVER`
/// uses.
pub const NOT_A_DRIVER: &str = "The person named is not an active driver of this tenant";
/// `specific_vehicle_id` does not name a vehicle of the demand's own tenant.
pub const NOT_A_TENANT_VEHICLE: &str = "The vehicle named does not belong to this tenant";

pub struct TransportDemandUseCase {
    gateway: TransportDemandGateway,
    users: UserGateway,
    vehicles: VehicleGateway,
}

impl TransportDemandUseCase {
    pub fn new(gateway: TransportDemandGateway, users: UserGateway, vehicles: VehicleGateway) -> Self {
        Self {
            gateway,
            users,
            vehicles,
        }
    }

    pub async fn create(&self, demand: TransportDemand) -> Result<TransportDemand, BusinessError> {
        let demand = self.validated(demand).await?;
        let entity = self.gateway.persist(demand).await.map_err(database_error)?;
        Ok(TransportDemandEntityMapper::from_active_model(entity))
    }

    pub async fn update(
        &self,
        id: i64,
        demand: TransportDemand,
    ) -> Result<TransportDemand, BusinessError> {
        let existing = self.find_by_id(id).await?;
        let demand = self.validated(demand).await?;

        let updated = TransportDemand {
            id: Some(id),
            uuid: existing.uuid,
            tenant_id: existing.tenant_id,
            created_at: existing.created_at,
            created_by: existing.created_by,
            updated_at: None,
            ..demand
        };

        let entity = self
            .gateway
            .persist(updated)
            .await
            .map_err(database_error)?;
        Ok(TransportDemandEntityMapper::from_active_model(entity))
    }

    /// A demand without a type is not a usable record. `specific_driver_id`
    /// (if named) must be an active `Driver` of the demand's own tenant;
    /// `specific_vehicle_id` (if named) must belong to it too -- the same
    /// boundary `vehicle_assignment_use_case::assign` enforces, applied here
    /// because nothing else validates these FKs before the write.
    async fn validated(&self, demand: TransportDemand) -> Result<TransportDemand, BusinessError> {
        let demand_type = demand.demand_type.trim().to_string();
        if demand_type.is_empty() {
            let msg = "Demand type is required".to_string();
            log::error!("[TransportDemandUseCase::validated] {}", msg);
            return Err(BusinessError::new(msg));
        }

        if let Some(driver_id) = demand.specific_driver_id {
            let driver = self
                .users
                .find_by_id(driver_id)
                .await
                .map_err(database_error)?;
            let is_valid_driver = driver.is_some_and(|user| {
                user.role == Role::Driver.to_string()
                    && user.enabled
                    && user.tenant_id == demand.tenant_id
            });
            if !is_valid_driver {
                return Err(BusinessError::new(NOT_A_DRIVER.to_string()));
            }
        }

        if let Some(vehicle_id) = demand.specific_vehicle_id {
            let vehicle = self
                .vehicles
                .find_by_id(vehicle_id)
                .await
                .map_err(database_error)?;
            let belongs_to_tenant =
                vehicle.is_some_and(|v| v.tenant_id == demand.tenant_id);
            if !belongs_to_tenant {
                return Err(BusinessError::new(NOT_A_TENANT_VEHICLE.to_string()));
            }
        }

        Ok(TransportDemand {
            demand_type,
            ..demand
        })
    }

    pub async fn find_by_id(&self, id: i64) -> Result<TransportDemand, BusinessError> {
        let entity = self.gateway.find_by_id(id).await.map_err(database_error)?;
        match entity {
            Some(model) => Ok(TransportDemandEntityMapper::from_model(model)),
            None => Err(BusinessError::new("Transport demand not found".to_string())),
        }
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<TransportDemand, BusinessError> {
        let entity = self
            .gateway
            .find_by_uuid(uuid)
            .await
            .map_err(database_error)?;
        match entity {
            Some(model) => Ok(TransportDemandEntityMapper::from_model(model)),
            None => Err(BusinessError::new("Transport demand not found".to_string())),
        }
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<TransportDemand>, u64), BusinessError> {
        let (entities, total) = self
            .gateway
            .find_page(page, page_size, search)
            .await
            .map_err(database_error)?;
        Ok((TransportDemandEntityMapper::from_models(entities), total))
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[TransportDemandUseCase] {}", msg);
    BusinessError::new(msg)
}
