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

impl FuelEntryUseCase {
    /// `TRM-1553`: the fuelling list plus its litre and value totals.
    pub async fn report(
        &self,
        from: Option<chrono::NaiveDateTime>,
        to: Option<chrono::NaiveDateTime>,
        vehicle_id: Option<i64>,
        station: Option<String>,
    ) -> Result<(Vec<FuelEntry>, f64, i64), BusinessError> {
        let rows = self
            .gateway
            .find_report(from, to, vehicle_id, station)
            .await
            .map_err(database_error)?;
        let entries = FuelEntryEntityMapper::from_models(rows);
        let (liters, cents) = totals(&entries);
        Ok((entries, liters, cents))
    }
}

fn totals(entries: &[FuelEntry]) -> (f64, i64) {
    entries
        .iter()
        .fold((0.0, 0), |(l, c), e| (l + e.volume_liters, c + e.value_cents))
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[FuelEntryUseCase] {}", msg);
    BusinessError::new(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::enums::FuelEntryOrigin;

    fn entry(liters: f64, cents: i64) -> FuelEntry {
        FuelEntry {
            id: None,
            uuid: None,
            tenant_id: Some(1),
            vehicle_id: 1,
            recorded_at: chrono::NaiveDateTime::default(),
            volume_liters: liters,
            value_cents: cents,
            odometer_km: None,
            station: None,
            full_tank: false,
            origin: FuelEntryOrigin::Manual,
            provider_transaction_id: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }
    }

    #[test]
    fn totals_sum_litres_and_value() {
        assert_eq!(totals(&[]), (0.0, 0));
        assert_eq!(totals(&[entry(40.5, 25000), entry(10.0, 6000)]), (50.5, 31000));
    }
}
