use crate::domain::business_error::BusinessError;
use crate::domain::km_evolution::KmEvolution;
use crate::gateway::km_evolution_gateway::KmEvolutionStore;
use crate::gateway::vehicle_gateway::VehicleOdometerStore;

pub struct KmEvolutionUseCase {
    gateway: Box<dyn KmEvolutionStore>,
    vehicles: Box<dyn VehicleOdometerStore>,
}

impl KmEvolutionUseCase {
    pub fn new<G, V>(gateway: G, vehicles: V) -> Self
    where
        G: KmEvolutionStore + 'static,
        V: VehicleOdometerStore + 'static,
    {
        Self {
            gateway: Box::new(gateway),
            vehicles: Box::new(vehicles),
        }
    }

    /// `AD-041`/`TRM-155`: recording a reading and recomputing
    /// `vehicle.odometer_km` are one operation, never two -- a caller that
    /// could do the first without the second is exactly the second writer
    /// `D-29` found.
    pub async fn create(&self, reading: KmEvolution) -> Result<KmEvolution, BusinessError> {
        let reading = Self::validated(reading)?;
        let saved = self.gateway.save(reading).await?;
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
        let Some(latest) = self.gateway.get_latest_by_vehicle(vehicle_id).await? else {
            return Ok(());
        };

        let Some(existing) = self.vehicles.get_by_id(vehicle_id).await? else {
            return Ok(());
        };

        let mut vehicle = existing;
        vehicle.odometer_km = Some(latest.km);
        self.vehicles.save(vehicle).await?;
        Ok(())
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<KmEvolution, BusinessError> {
        self.gateway
            .get_by_uuid(uuid)
            .await?
            .ok_or_else(|| BusinessError::new("Kilometre reading not found".to_string()))
    }

    pub async fn history(
        &self,
        vehicle_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<KmEvolution>, u64), BusinessError> {
        self.gateway
            .list_by_vehicle(vehicle_id, page, page_size)
            .await
    }
}
