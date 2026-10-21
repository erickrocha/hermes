use business::domain::business_error::BusinessError;
use business::domain::daily_schedule::DailySchedule;
use business::domain::enums::Role;
use business::domain::user::User;
use business::domain::vehicle::Vehicle;
use business::gateway::daily_schedule_gateway::{DailyScheduleStore, DailyScheduleStoreError};
use business::gateway::user_gateway::UserLookupStore;
use business::gateway::vehicle_gateway::VehicleLookupStore;
use business::use_cases::daily_schedule_use_case::{
    DailyScheduleUseCase, NOT_A_DRIVER, NOT_A_TENANT_VEHICLE,
};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};

struct MemoryScheduleStore {
    writes: AtomicUsize,
    deletes: AtomicUsize,
    duplicate: bool,
    existing: Option<DailySchedule>,
}

#[async_trait]
impl DailyScheduleStore for MemoryScheduleStore {
    async fn save(&self, entry: DailySchedule) -> Result<DailySchedule, DailyScheduleStoreError> {
        self.writes.fetch_add(1, Ordering::Relaxed);
        if self.duplicate {
            Err(DailyScheduleStoreError::Duplicate)
        } else {
            Ok(entry)
        }
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<DailySchedule>, BusinessError> {
        Ok(self
            .existing
            .clone()
            .filter(|entry| entry.uuid.as_deref() == Some(uuid.as_str())))
    }

    async fn delete_by_id(&self, _id: i64) -> Result<(), BusinessError> {
        self.deletes.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn list_page_by_demand(
        &self,
        _demand_id: i64,
        _page: u64,
        _page_size: u64,
    ) -> Result<(Vec<DailySchedule>, u64), BusinessError> {
        Ok((
            self.existing.clone().into_iter().collect(),
            if self.existing.is_some() { 1 } else { 0 },
        ))
    }
}

struct MemoryUsers {
    user: Option<User>,
}

#[async_trait]
impl UserLookupStore for MemoryUsers {
    async fn get_by_id(&self, id: i64) -> Result<Option<User>, BusinessError> {
        Ok(self
            .user
            .as_ref()
            .filter(|user| user.id == Some(id))
            .cloned())
    }

    async fn get_by_uuid(&self, _uuid: String) -> Result<Option<User>, BusinessError> {
        Ok(None)
    }
}

struct NoVehicles;

#[async_trait]
impl VehicleLookupStore for NoVehicles {
    async fn get_by_id(
        &self,
        _id: i64,
    ) -> Result<Option<business::domain::vehicle::Vehicle>, BusinessError> {
        Ok(None)
    }
}

struct MemoryVehicles(Option<Vehicle>);

#[async_trait]
impl VehicleLookupStore for MemoryVehicles {
    async fn get_by_id(&self, _id: i64) -> Result<Option<Vehicle>, BusinessError> {
        Ok(self.0.clone())
    }
}

fn vehicle() -> Vehicle {
    Vehicle {
        id: Some(10),
        uuid: Some("vehicle-10".into()),
        tenant_id: Some(42),
        plate: "ABC123".into(),
        model: "Truck".into(),
        status: business::domain::enums::VehicleStatus::Active,
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

fn store(duplicate: bool, existing: Option<DailySchedule>) -> MemoryScheduleStore {
    MemoryScheduleStore {
        writes: AtomicUsize::new(0),
        deletes: AtomicUsize::new(0),
        duplicate,
        existing,
    }
}

fn user(role: Role) -> User {
    User {
        id: Some(5),
        uuid: Some("driver-5".into()),
        email: "driver@example.com".into(),
        name: Some("Driver".into()),
        password: String::new(),
        enabled: true,
        tenant_id: Some(42),
        role,
        cpf: None,
        principal_function: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn entry() -> DailySchedule {
    DailySchedule {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        demand_id: 50,
        driver_id: 5,
        vehicle_id: 10,
        date: chrono::NaiveDate::from_ymd_opt(2026, 1, 5).expect("valid fixed date"),
        start_time: None,
        end_time: None,
        notes: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn a_non_driver_is_rejected_before_vehicle_lookup_or_write() {
    let schedules = store(false, None);
    let use_case = DailyScheduleUseCase::new(
        schedules,
        MemoryUsers {
            user: Some(user(Role::TenantUser)),
        },
        NoVehicles,
    );

    let error = use_case
        .create(entry())
        .await
        .expect_err("tenant user is not a driver");

    assert_eq!(error.message, NOT_A_DRIVER);
}

#[tokio::test]
async fn a_driver_with_no_tenant_vehicle_is_rejected_before_write() {
    let schedules = store(false, None);
    let use_case = DailyScheduleUseCase::new(
        schedules,
        MemoryUsers {
            user: Some(user(Role::Driver)),
        },
        NoVehicles,
    );

    let error = use_case
        .create(entry())
        .await
        .expect_err("vehicle does not exist in tenant");

    assert_eq!(error.message, NOT_A_TENANT_VEHICLE);
}

#[tokio::test]
async fn a_valid_driver_and_vehicle_create_the_schedule_entry() {
    let schedules = store(false, None);
    let use_case = DailyScheduleUseCase::new(
        schedules,
        MemoryUsers {
            user: Some(user(Role::Driver)),
        },
        MemoryVehicles(Some(vehicle())),
    );

    let saved = use_case
        .create(entry())
        .await
        .expect("same-tenant driver and vehicle are accepted");

    assert_eq!(saved.demand_id, 50);
}

#[tokio::test]
async fn duplicate_schedule_entries_are_mapped_to_a_domain_error() {
    let use_case = DailyScheduleUseCase::new(
        store(true, None),
        MemoryUsers {
            user: Some(user(Role::Driver)),
        },
        MemoryVehicles(Some(vehicle())),
    );

    let error = use_case
        .create(entry())
        .await
        .expect_err("the store's unique-key conflict is reported");

    assert_eq!(
        error.message,
        business::use_cases::daily_schedule_use_case::DUPLICATE_SCHEDULE_ENTRY
    );
}

#[tokio::test]
async fn schedule_lookup_remove_and_history_use_the_expected_demand_scope() {
    let mut existing = entry();
    existing.id = Some(15);
    existing.uuid = Some("entry-15".into());
    let schedules = store(false, Some(existing.clone()));
    let use_case = DailyScheduleUseCase::new(schedules, MemoryUsers { user: None }, NoVehicles);

    assert_eq!(
        use_case
            .find_by_uuid("entry-15".into())
            .await
            .expect("the known entry is found")
            .id,
        Some(15)
    );
    assert_eq!(
        use_case
            .find_by_uuid("missing".into())
            .await
            .expect_err("unknown entries are not found")
            .message,
        "Schedule entry not found"
    );
    assert_eq!(
        use_case
            .remove(99, "entry-15".into())
            .await
            .expect_err("an entry cannot be removed through a different demand")
            .message,
        "Schedule entry not found"
    );
    use_case
        .remove(50, "entry-15".into())
        .await
        .expect("the owning demand can remove the entry");
    let (rows, total) = use_case
        .history(50, 0, 20)
        .await
        .expect("history is returned by demand");
    assert_eq!(rows.len(), 1);
    assert_eq!(total, 1);
}
