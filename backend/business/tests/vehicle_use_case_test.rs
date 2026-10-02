use business::domain::business_error::BusinessError;
use business::domain::enums::{GarageTagOrigin, VehicleStatus};
use business::domain::vehicle::Vehicle;
use business::gateway::vehicle_gateway::{VehicleStore, VehicleStoreError};
use business::use_cases::vehicle_use_case::{
    DUPLICATE_PLATE, DUPLICATE_TRACKER_DEVICE, VehicleUseCase,
};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryVehicleStore {
    vehicles: Mutex<Vec<Vehicle>>,
}

#[async_trait]
impl VehicleStore for MemoryVehicleStore {
    async fn save(&self, mut vehicle: Vehicle) -> Result<Vehicle, VehicleStoreError> {
        let mut vehicles = self.vehicles.lock().map_err(|_| {
            VehicleStoreError::Storage(BusinessError::new("memory store poisoned".into()))
        })?;
        if vehicles.iter().any(|existing| {
            existing.id != vehicle.id
                && existing.tenant_id == vehicle.tenant_id
                && existing.plate == vehicle.plate
        }) {
            return Err(VehicleStoreError::DuplicatePlate);
        }
        if let Some(device_id) = vehicle.tracker_device_id
            && vehicles.iter().any(|existing| {
                existing.id != vehicle.id && existing.tracker_device_id == Some(device_id)
            })
        {
            return Err(VehicleStoreError::DuplicateTrackerDevice);
        }
        match vehicle.id {
            Some(id) => {
                if let Some(existing) = vehicles.iter_mut().find(|existing| existing.id == Some(id))
                {
                    *existing = vehicle.clone();
                } else {
                    vehicles.push(vehicle.clone());
                }
            }
            None => {
                let id = vehicles.len() as i64 + 1;
                vehicle.id = Some(id);
                vehicle.uuid = Some(format!("vehicle-{id}"));
                vehicles.push(vehicle.clone());
            }
        }
        Ok(vehicle)
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<Vehicle>, BusinessError> {
        let vehicles = self
            .vehicles
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(vehicles
            .iter()
            .find(|vehicle| vehicle.id == Some(id))
            .cloned())
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<Vehicle>, BusinessError> {
        let vehicles = self
            .vehicles
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(vehicles
            .iter()
            .find(|vehicle| vehicle.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }

    async fn find_by_plate(
        &self,
        plate: &str,
        tenant_id: Option<i64>,
    ) -> Result<Option<Vehicle>, BusinessError> {
        let vehicles = self
            .vehicles
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(vehicles
            .iter()
            .find(|vehicle| vehicle.plate == plate && vehicle.tenant_id == tenant_id)
            .cloned())
    }

    async fn find_by_tracker_device(
        &self,
        device_id: i64,
    ) -> Result<Option<Vehicle>, BusinessError> {
        let vehicles = self
            .vehicles
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(vehicles
            .iter()
            .find(|vehicle| vehicle.tracker_device_id == Some(device_id))
            .cloned())
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<Vehicle>, u64), BusinessError> {
        let vehicles = self
            .vehicles
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let filtered: Vec<_> = vehicles
            .iter()
            .filter(|vehicle| {
                search
                    .is_none_or(|term| vehicle.plate.contains(term) || vehicle.model.contains(term))
            })
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

    async fn list_all(&self) -> Result<Vec<Vehicle>, BusinessError> {
        let vehicles = self
            .vehicles
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(vehicles.clone())
    }
}

fn vehicle(plate: &str, model: &str, tenant_id: Option<i64>) -> Vehicle {
    Vehicle {
        id: None,
        uuid: None,
        tenant_id,
        plate: plate.into(),
        model: model.into(),
        status: VehicleStatus::Active,
        tracker_device_id: None,
        prefix: None,
        vehicle_type: None,
        odometer_km: None,
        wheel_type: None,
        spare_tire_count: None,
        spare_tire_type: None,
        spare_tire_notes: None,
        tank_capacity_liters: None,
        reference_km_per_liter: None,
        garage_tag: None,
        garage_tag_origin: Option::<GarageTagOrigin>::None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn vehicle_use_case_normalizes_creates_finds_and_pages_through_its_store() {
    let use_case = VehicleUseCase::new(MemoryVehicleStore::default());

    let created = use_case
        .create(vehicle(" abc1d23 ", "  Truck  ", Some(42)))
        .await
        .expect("valid vehicle is stored");
    let found = use_case
        .find_by_uuid(created.uuid.clone().expect("store assigned uuid"))
        .await
        .expect("vehicle is found");
    let (page, total) = use_case
        .find_page(0, 10, Some("ABC"))
        .await
        .expect("matching vehicles are listed");

    assert_eq!(created.plate, "ABC1D23");
    assert_eq!(created.model, "Truck");
    assert_eq!(found.id, created.id);
    assert_eq!(total, 1);
    assert_eq!(page.len(), 1);
}

#[tokio::test]
async fn vehicle_use_case_keeps_tenant_and_uuid_when_updating() {
    let use_case = VehicleUseCase::new(MemoryVehicleStore::default());
    let created = use_case
        .create(vehicle("ABC1D23", "Truck", Some(42)))
        .await
        .expect("vehicle is stored");
    let id = created.id.expect("store assigned id");
    let uuid = created.uuid.clone();
    let updated = use_case
        .update(id, vehicle("XYZ4E56", "Updated truck", Some(99)))
        .await
        .expect("vehicle is updated");

    assert_eq!(updated.tenant_id, Some(42));
    assert_eq!(updated.uuid, uuid);
    assert_eq!(updated.plate, "XYZ4E56");
}

#[tokio::test]
async fn vehicle_use_case_maps_duplicate_plate_and_tracker_conflicts() {
    let use_case = VehicleUseCase::new(MemoryVehicleStore::default());
    use_case
        .create(vehicle("ABC1D23", "First", Some(42)))
        .await
        .expect("first vehicle is stored");
    let duplicate_plate = use_case
        .create(vehicle(" abc1d23 ", "Second", Some(42)))
        .await
        .expect_err("normalized duplicate plate is refused");
    assert_eq!(duplicate_plate.message, DUPLICATE_PLATE);

    let mut tracker_vehicle = vehicle("XYZ4E56", "Tracker", Some(42));
    tracker_vehicle.tracker_device_id = Some(77);
    use_case
        .create(tracker_vehicle.clone())
        .await
        .expect("tracked vehicle is stored");
    tracker_vehicle.plate = "QRS9T87".into();
    let duplicate_tracker = use_case
        .create(tracker_vehicle)
        .await
        .expect_err("tracker linked to another vehicle is refused");

    assert_eq!(duplicate_tracker.message, DUPLICATE_TRACKER_DEVICE);
}
