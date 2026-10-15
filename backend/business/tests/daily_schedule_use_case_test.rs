use business::domain::business_error::BusinessError;
use business::domain::daily_schedule::DailySchedule;
use business::domain::enums::Role;
use business::domain::user::User;
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
}

#[async_trait]
impl DailyScheduleStore for MemoryScheduleStore {
    async fn save(&self, _entry: DailySchedule) -> Result<DailySchedule, DailyScheduleStoreError> {
        self.writes.fetch_add(1, Ordering::Relaxed);
        Err(DailyScheduleStoreError::Duplicate)
    }

    async fn get_by_uuid(&self, _uuid: String) -> Result<Option<DailySchedule>, BusinessError> {
        Ok(None)
    }

    async fn delete_by_id(&self, _id: i64) -> Result<(), BusinessError> {
        Ok(())
    }

    async fn list_page_by_demand(
        &self,
        _demand_id: i64,
        _page: u64,
        _page_size: u64,
    ) -> Result<(Vec<DailySchedule>, u64), BusinessError> {
        Ok((Vec::new(), 0))
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
    let schedules = MemoryScheduleStore {
        writes: AtomicUsize::new(0),
    };
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
    let schedules = MemoryScheduleStore {
        writes: AtomicUsize::new(0),
    };
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
