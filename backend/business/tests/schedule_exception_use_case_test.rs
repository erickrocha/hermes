use business::domain::business_error::BusinessError;
use business::domain::enums::{Role, ScheduleExceptionType, VehicleStatus};
use business::domain::schedule_exception::ScheduleException;
use business::domain::user::User;
use business::domain::vehicle::Vehicle;
use business::gateway::schedule_exception_gateway::ScheduleExceptionStore;
use business::gateway::user_gateway::UserLookupStore;
use business::gateway::vehicle_gateway::VehicleLookupStore;
use business::use_cases::schedule_exception_use_case::{
    NOT_A_DRIVER, NOT_A_TENANT_VEHICLE, ScheduleExceptionUseCase,
};
use chrono::NaiveDate;
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryScheduleExceptionStore {
    exceptions: Mutex<Vec<ScheduleException>>,
}

#[async_trait]
impl ScheduleExceptionStore for MemoryScheduleExceptionStore {
    async fn save(
        &self,
        mut exception: ScheduleException,
    ) -> Result<ScheduleException, BusinessError> {
        let mut exceptions = self
            .exceptions
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let id = exception.id.unwrap_or(exceptions.len() as i64 + 1);
        exception.id = Some(id);
        exception.uuid = Some(format!("schedule-exception-{id}"));
        exceptions.push(exception.clone());
        Ok(exception)
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<ScheduleException>, BusinessError> {
        let exceptions = self
            .exceptions
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(exceptions
            .iter()
            .find(|exception| exception.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }

    async fn delete_by_id(&self, id: i64) -> Result<(), BusinessError> {
        let mut exceptions = self
            .exceptions
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        exceptions.retain(|exception| exception.id != Some(id));
        Ok(())
    }

    async fn list_by_demand(
        &self,
        demand_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<ScheduleException>, u64), BusinessError> {
        let exceptions = self
            .exceptions
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let filtered: Vec<_> = exceptions
            .iter()
            .filter(|exception| exception.demand_id == demand_id)
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

struct MemoryUserStore {
    user: Option<User>,
}

#[async_trait]
impl UserLookupStore for MemoryUserStore {
    async fn get_by_id(&self, id: i64) -> Result<Option<User>, BusinessError> {
        Ok(self
            .user
            .as_ref()
            .filter(|user| user.id == Some(id))
            .cloned())
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<User>, BusinessError> {
        Ok(self
            .user
            .as_ref()
            .filter(|user| user.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }
}

struct MemoryVehicleStore {
    vehicle: Option<Vehicle>,
}

#[async_trait]
impl VehicleLookupStore for MemoryVehicleStore {
    async fn get_by_id(&self, id: i64) -> Result<Option<Vehicle>, BusinessError> {
        Ok(self
            .vehicle
            .as_ref()
            .filter(|vehicle| vehicle.id == Some(id))
            .cloned())
    }
}

fn exception() -> ScheduleException {
    ScheduleException {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        demand_id: 5,
        date: NaiveDate::from_ymd_opt(2026, 10, 2).expect("valid test date"),
        exception_type: ScheduleExceptionType::Cancellation,
        new_driver_id: None,
        new_vehicle_id: None,
        reason: Some("Cancelled".into()),
        extra_trip_id: None,
        status: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn driver(tenant_id: Option<i64>, role: Role, enabled: bool) -> User {
    User {
        id: Some(7),
        uuid: Some("driver-7".into()),
        email: "driver@example.com".into(),
        name: Some("Driver".into()),
        password: "hash".into(),
        enabled,
        tenant_id,
        role,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn vehicle(tenant_id: Option<i64>) -> Vehicle {
    Vehicle {
        id: Some(10),
        uuid: Some("vehicle-10".into()),
        tenant_id,
        plate: "ABC1D23".into(),
        model: "Truck".into(),
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
        garage_tag_origin: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn use_case(user: Option<User>, vehicle: Option<Vehicle>) -> ScheduleExceptionUseCase {
    ScheduleExceptionUseCase::new(
        MemoryScheduleExceptionStore::default(),
        MemoryUserStore { user },
        MemoryVehicleStore { vehicle },
    )
}

#[tokio::test]
async fn schedule_exception_validates_driver_and_vehicle_then_saves() {
    let mut new_exception = exception();
    new_exception.new_driver_id = Some(7);
    new_exception.new_vehicle_id = Some(10);
    let created = use_case(
        Some(driver(Some(42), Role::Driver, true)),
        Some(vehicle(Some(42))),
    )
    .create(new_exception)
    .await
    .expect("same-tenant driver and vehicle are accepted");

    assert_eq!(created.new_driver_id, Some(7));
    assert_eq!(created.new_vehicle_id, Some(10));
}

#[tokio::test]
async fn schedule_exception_rejects_a_driver_from_another_tenant() {
    let mut new_exception = exception();
    new_exception.new_driver_id = Some(7);
    let error = use_case(
        Some(driver(Some(99), Role::Driver, true)),
        Some(vehicle(Some(42))),
    )
    .create(new_exception)
    .await
    .expect_err("cross-tenant driver is refused");

    assert_eq!(error.message, NOT_A_DRIVER);
}

#[tokio::test]
async fn schedule_exception_rejects_a_vehicle_from_another_tenant() {
    let mut new_exception = exception();
    new_exception.new_vehicle_id = Some(10);
    let error = use_case(
        Some(driver(Some(42), Role::Driver, true)),
        Some(vehicle(Some(99))),
    )
    .create(new_exception)
    .await
    .expect_err("cross-tenant vehicle is refused");

    assert_eq!(error.message, NOT_A_TENANT_VEHICLE);
}
