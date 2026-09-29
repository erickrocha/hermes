use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::fuel_entry::{FuelEntry, FuelEntryEntityMapper};
use crate::gateway::fuel_entry_gateway::FuelEntryGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use sea_orm::DbErr;

pub const VOLUME_MUST_BE_POSITIVE: &str = "A fuelling's volume must be positive";
pub const VALUE_MUST_NOT_BE_NEGATIVE: &str = "A fuelling's value cannot be negative";
pub const VEHICLE_NOT_FOUND: &str = "Vehicle not found";
pub const FUEL_ENTRY_NOT_FOUND: &str = "Fuel entry not found";

pub struct FuelEntryUseCase {
    gateway: FuelEntryGateway,
    vehicles: VehicleGateway,
}

impl FuelEntryUseCase {
    pub fn new(gateway: FuelEntryGateway, vehicles: VehicleGateway) -> Self {
        Self { gateway, vehicles }
    }

    pub async fn create(&self, entry: FuelEntry) -> Result<FuelEntry, BusinessError> {
        let entry = self.validated(entry).await?;
        let entity = self.gateway.persist(entry).await.map_err(database_error)?;
        Ok(FuelEntryEntityMapper::from_active_model(entity))
    }

    async fn validated(&self, entry: FuelEntry) -> Result<FuelEntry, BusinessError> {
        if entry.volume_liters <= 0.0 {
            return Err(BusinessError::new(VOLUME_MUST_BE_POSITIVE.to_string()));
        }
        if entry.value_cents < 0 {
            return Err(BusinessError::new(VALUE_MUST_NOT_BE_NEGATIVE.to_string()));
        }

        let vehicle = self
            .vehicles
            .find_by_id(entry.vehicle_id)
            .await
            .map_err(database_error)?;
        if !vehicle.is_some_and(|v| v.tenant_id == entry.tenant_id) {
            return Err(BusinessError::new(VEHICLE_NOT_FOUND.to_string()));
        }

        Ok(entry)
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<FuelEntry, BusinessError> {
        let entity = self.gateway.find_by_uuid(uuid).await.map_err(database_error)?;
        match entity {
            Some(model) => Ok(FuelEntryEntityMapper::from_model(model)),
            None => Err(BusinessError::new(FUEL_ENTRY_NOT_FOUND.to_string())),
        }
    }

    pub async fn find_page(&self, page: u64, page_size: u64) -> Result<(Vec<FuelEntry>, u64), BusinessError> {
        let (rows, total) = self.gateway.find_page(page, page_size).await.map_err(database_error)?;
        Ok((FuelEntryEntityMapper::from_models(rows), total))
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[FuelEntryUseCase] {}", msg);
    BusinessError::new(msg)
}
