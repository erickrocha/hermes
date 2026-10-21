use business::domain::business_error::BusinessError;
use business::domain::enums::{KmOrigin, VehicleStatus};
use business::domain::km_evolution::KmEvolution;
use business::domain::vehicle::Vehicle;
use business::gateway::km_evolution_gateway::KmEvolutionStore;
use business::gateway::vehicle_gateway::VehicleOdometerStore;
use business::use_cases::km_evolution_use_case::KmEvolutionUseCase;
use chrono::{NaiveDate, NaiveDateTime, Utc};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct MemoryKmEvolutionStore {
    readings: Arc<Mutex<Vec<KmEvolution>>>,
}

#[async_trait]
impl KmEvolutionStore for MemoryKmEvolutionStore {
    async fn save(&self, mut reading: KmEvolution) -> Result<KmEvolution, BusinessError> {
        let mut readings = self
            .readings
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let id = reading.id.unwrap_or(readings.len() as i64 + 1);
        reading.id = Some(id);
        reading.uuid = Some(format!("reading-{id}"));
        reading
            .recorded_at
            .get_or_insert_with(|| Utc::now().naive_utc());
        readings.push(reading.clone());
        Ok(reading)
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<KmEvolution>, BusinessError> {
        let readings = self
            .readings
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(readings
            .iter()
            .find(|reading| reading.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }

    async fn get_latest_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<KmEvolution>, BusinessError> {
        let readings = self
            .readings
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(readings
            .iter()
            .filter(|reading| reading.vehicle_id == vehicle_id)
            .max_by_key(|reading| (reading.recorded_at, reading.id))
            .cloned())
    }

    async fn get_before(
        &self,
        vehicle_id: i64,
        at: NaiveDateTime,
    ) -> Result<Option<KmEvolution>, BusinessError> {
        let readings = self
            .readings
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(readings
            .iter()
            .filter(|reading| {
                reading.vehicle_id == vehicle_id
                    && reading
                        .recorded_at
                        .is_some_and(|recorded_at| recorded_at <= at)
            })
            .max_by_key(|reading| (reading.recorded_at, reading.id))
            .cloned())
    }

    async fn get_after(
        &self,
        vehicle_id: i64,
        at: NaiveDateTime,
    ) -> Result<Option<KmEvolution>, BusinessError> {
        let readings = self
            .readings
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(readings
            .iter()
            .filter(|reading| {
                reading.vehicle_id == vehicle_id
                    && reading
                        .recorded_at
                        .is_some_and(|recorded_at| recorded_at > at)
            })
            .min_by_key(|reading| (reading.recorded_at, reading.id))
            .cloned())
    }

    async fn list_by_vehicle(
        &self,
        vehicle_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<KmEvolution>, u64), BusinessError> {
        let readings = self
            .readings
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let filtered: Vec<_> = readings
            .iter()
            .filter(|reading| reading.vehicle_id == vehicle_id)
            .cloned()
            .collect();
        let total = filtered.len() as u64;
        let start = (page * page_size) as usize;
        Ok((
            filtered
                .into_iter()
                .skip(start)
                .take(page_size as usize)
                .collect(),
            total,
        ))
    }
}

#[derive(Clone)]
struct MemoryVehicleOdometerStore {
    vehicle: Arc<Mutex<Option<Vehicle>>>,
}

#[async_trait]
impl VehicleOdometerStore for MemoryVehicleOdometerStore {
    async fn get_by_id(&self, id: i64) -> Result<Option<Vehicle>, BusinessError> {
        let vehicle = self
            .vehicle
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(vehicle
            .as_ref()
            .filter(|vehicle| vehicle.id == Some(id))
            .cloned())
    }

    async fn save(&self, vehicle: Vehicle) -> Result<Vehicle, BusinessError> {
        let mut stored = self
            .vehicle
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        *stored = Some(vehicle.clone());
        Ok(vehicle)
    }
}

fn at(day: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 10, day)
        .expect("valid test date")
        .and_hms_opt(12, 0, 0)
        .expect("valid test time")
}

fn reading(km: f64, recorded_at: NaiveDateTime) -> KmEvolution {
    KmEvolution {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        vehicle_id: 10,
        km,
        recorded_at: Some(recorded_at),
        origin: KmOrigin::Adjustment,
        source_entity: None,
        source_entity_id: None,
        notes: None,
        recorded_by_user_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn vehicle() -> Vehicle {
    Vehicle {
        id: Some(10),
        uuid: Some("vehicle-10".into()),
        tenant_id: Some(42),
        plate: "ABC1D23".into(),
        model: "Truck".into(),
        status: VehicleStatus::Active,
        tracker_device_id: None,
        prefix: None,
        vehicle_type: None,
        odometer_km: Some(0.0),
        wheel_type: None,
        spare_tire_count: None,
        spare_tire_type: None,
        spare_tire_notes: None,
        tank_capacity_liters: None,
        reference_km_per_liter: None,
        garage_tag: None,
        garage_tag_origin: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn use_case(
    readings: MemoryKmEvolutionStore,
    vehicle: MemoryVehicleOdometerStore,
) -> KmEvolutionUseCase {
    KmEvolutionUseCase::new(readings, vehicle)
}

#[tokio::test]
async fn a_backdated_reading_does_not_replace_the_vehicle_official_odometer() {
    let readings = MemoryKmEvolutionStore::default();
    readings
        .save(reading(15_000.0, at(20)))
        .await
        .expect("latest reading is seeded");
    let vehicle_store = MemoryVehicleOdometerStore {
        vehicle: Arc::new(Mutex::new(Some(vehicle()))),
    };
    let use_case = use_case(readings, vehicle_store.clone());

    let created = use_case
        .create(reading(12_000.0, at(10)))
        .await
        .expect("backdated reading is recorded");
    let updated_vehicle = vehicle_store
        .get_by_id(10)
        .await
        .expect("vehicle lookup succeeds")
        .expect("vehicle exists");

    assert_eq!(created.km, 12_000.0);
    assert_eq!(updated_vehicle.odometer_km, Some(15_000.0));
}

#[tokio::test]
async fn a_new_latest_reading_updates_the_vehicle_odometer() {
    let readings = MemoryKmEvolutionStore::default();
    let vehicle_store = MemoryVehicleOdometerStore {
        vehicle: Arc::new(Mutex::new(Some(vehicle()))),
    };
    let use_case = use_case(readings, vehicle_store.clone());

    use_case
        .create(reading(16_000.0, at(20)))
        .await
        .expect("reading is recorded");
    let updated_vehicle = vehicle_store
        .get_by_id(10)
        .await
        .expect("vehicle lookup succeeds")
        .expect("vehicle exists");

    assert_eq!(updated_vehicle.odometer_km, Some(16_000.0));
}

#[tokio::test]
async fn a_negative_reading_is_rejected_before_storage() {
    let readings = MemoryKmEvolutionStore::default();
    let use_case = use_case(
        readings.clone(),
        MemoryVehicleOdometerStore {
            vehicle: Arc::new(Mutex::new(Some(vehicle()))),
        },
    );

    let error = use_case
        .create(reading(-1.0, at(20)))
        .await
        .expect_err("negative odometer is refused");

    assert_eq!(error.message, "Odometer reading cannot be negative");
    assert!(
        readings
            .get_latest_by_vehicle(10)
            .await
            .expect("lookup succeeds")
            .is_none()
    );
}

#[tokio::test]
async fn readings_remain_queryable_when_the_vehicle_is_missing() {
    let readings = MemoryKmEvolutionStore::default();
    let use_case = use_case(
        readings,
        MemoryVehicleOdometerStore {
            vehicle: Arc::new(Mutex::new(None)),
        },
    );

    let created = use_case
        .create(reading(12_000.0, at(20)))
        .await
        .expect("reading is recorded even if vehicle no longer exists");
    let found = use_case
        .find_by_uuid(created.uuid.clone().unwrap())
        .await
        .expect("saved reading is retrievable");
    let (page, total) = use_case
        .history(10, 0, 20)
        .await
        .expect("reading history is available");

    assert_eq!(found.km, 12_000.0);
    assert_eq!(page, vec![created]);
    assert_eq!(total, 1);
    assert_eq!(
        use_case
            .find_by_uuid("missing".into())
            .await
            .expect_err("unknown reading is not found")
            .message,
        "Kilometre reading not found"
    );
}
