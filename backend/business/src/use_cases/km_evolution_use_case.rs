use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::km_evolution::{KmEvolution, KmEvolutionEntityMapper};
use crate::domain::vehicle::VehicleEntityMapper;
use crate::gateway::km_evolution_gateway::KmEvolutionGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use sea_orm::DbErr;

pub struct KmEvolutionUseCase {
    gateway: KmEvolutionGateway,
    vehicles: VehicleGateway,
}

impl KmEvolutionUseCase {
    pub fn new(gateway: KmEvolutionGateway, vehicles: VehicleGateway) -> Self {
        Self { gateway, vehicles }
    }

    /// `AD-041`/`TRM-155`: recording a reading and recomputing
    /// `vehicle.odometer_km` are one operation, never two -- a caller that
    /// could do the first without the second is exactly the second writer
    /// `D-29` found.
    pub async fn create(&self, reading: KmEvolution) -> Result<KmEvolution, BusinessError> {
        let reading = Self::validated(reading)?;
        let entity = self.gateway.persist(reading).await.map_err(database_error)?;
        let saved = KmEvolutionEntityMapper::from_active_model(entity);
        self.recompute_vehicle_odometer(saved.vehicle_id).await?;
        Ok(saved)
    }

    fn validated(reading: KmEvolution) -> Result<KmEvolution, BusinessError> {
        if reading.km < 0.0 {
            let msg = "Odometer reading cannot be negative".to_string();
            log::error!("[KmEvolutionUseCase::validated] {}", msg);
            return Err(BusinessError::new(msg));
        }
        Ok(reading)
    }

    /// Always re-derives from the `km_evolution` table itself rather than
    /// trusting the row just inserted -- a backdated `Adjustment` does not
    /// necessarily become the new official reading, and this is the one
    /// place that decides which row does (`find_latest_by_vehicle`).
    async fn recompute_vehicle_odometer(&self, vehicle_id: i64) -> Result<(), BusinessError> {
        let Some(latest) = self
            .gateway
            .find_latest_by_vehicle(vehicle_id)
            .await
            .map_err(database_error)?
        else {
            return Ok(());
        };

        let Some(existing) = self
            .vehicles
            .find_by_id(vehicle_id)
            .await
            .map_err(database_error)?
        else {
            return Ok(());
        };

        let mut vehicle = VehicleEntityMapper::from_model(existing);
        vehicle.odometer_km = Some(latest.km);
        self.vehicles.persist(vehicle).await.map_err(database_error)?;
        Ok(())
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<KmEvolution, BusinessError> {
        let entity = self
            .gateway
            .find_by_uuid(uuid)
            .await
            .map_err(database_error)?;
        match entity {
            Some(model) => Ok(KmEvolutionEntityMapper::from_model(model)),
            None => Err(BusinessError::new("Kilometre reading not found".to_string())),
        }
    }

    pub async fn history(
        &self,
        vehicle_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<KmEvolution>, u64), BusinessError> {
        let (rows, total) = self
            .gateway
            .find_page_by_vehicle(vehicle_id, page, page_size)
            .await
            .map_err(database_error)?;
        Ok((KmEvolutionEntityMapper::from_models(rows), total))
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[KmEvolutionUseCase] {}", msg);
    BusinessError::new(msg)
}
