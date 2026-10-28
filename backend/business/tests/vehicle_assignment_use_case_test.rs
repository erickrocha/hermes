use business::domain::business_error::BusinessError;
use business::domain::enums::{VehicleStatus};
use business::domain::user::{Access, User};
use business::domain::permission::{DRIVER, TENANT_USER};
use business::domain::vehicle::Vehicle;
use business::domain::vehicle_assignment::VehicleAssignment;
use business::gateway::user_gateway::UserLookupStore;
use business::gateway::vehicle_assignment_gateway::{
    VehicleAssignmentStore, VehicleAssignmentStoreError,
};
use business::use_cases::vehicle_assignment_use_case::{
    NOT_A_DRIVER, VEHICLE_ALREADY_ASSIGNED, VehicleAssignmentUseCase,
};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryAssignmentStore {
    assignments: Mutex<Vec<VehicleAssignment>>,
}

#[async_trait]
impl VehicleAssignmentStore for MemoryAssignmentStore {
    async fn save(
        &self,
        mut assignment: VehicleAssignment,
    ) -> Result<VehicleAssignment, VehicleAssignmentStoreError> {
        let mut assignments = self.assignments.lock().map_err(|_| {
            VehicleAssignmentStoreError::Storage(BusinessError::new("memory store poisoned".into()))
        })?;
        if assignment.is_live()
            && assignments.iter().any(|existing| {
                existing.vehicle_id == assignment.vehicle_id
                    && existing.is_live()
                    && existing.id != assignment.id
            })
        {
            return Err(VehicleAssignmentStoreError::AlreadyAssigned);
        }
        let id = assignment.id.unwrap_or(assignments.len() as i64 + 1);
        assignment.id = Some(id);
        assignment.uuid = Some(format!("assignment-{id}"));
        if let Some(existing) = assignments.iter_mut().find(|row| row.id == Some(id)) {
            *existing = assignment.clone();
        } else {
            assignments.push(assignment.clone());
        }
        Ok(assignment)
    }

    async fn get_live_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<VehicleAssignment>, BusinessError> {
        let assignments = self
            .assignments
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(assignments
            .iter()
            .find(|row| row.vehicle_id == vehicle_id && row.is_live())
            .cloned())
    }

    async fn list_by_vehicle(
        &self,
        vehicle_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<VehicleAssignment>, u64), BusinessError> {
        let assignments = self
            .assignments
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let filtered: Vec<_> = assignments
            .iter()
            .filter(|row| row.vehicle_id == vehicle_id)
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

#[derive(Default)]
struct MemoryUserStore {
    users: Mutex<Vec<User>>,
}

#[async_trait]
impl UserLookupStore for MemoryUserStore {
    async fn get_by_id(&self, id: i64) -> Result<Option<User>, BusinessError> {
        let users = self
            .users
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(users.iter().find(|user| user.id == Some(id)).cloned())
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<User>, BusinessError> {
        let users = self
            .users
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(users
            .iter()
            .find(|user| user.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }
}

fn vehicle(id: Option<i64>, tenant_id: Option<i64>) -> Vehicle {
    Vehicle {
        id,
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

fn driver() -> User {
    User {
        id: Some(5),
        uuid: Some("driver-5".into()),
        email: "driver@example.com".into(),
        name: Some("Driver".into()),
        password: "hash".into(),
        enabled: true,
        tenant_id: Some(42),
        profile_id: Some(1),
        extra_permissions: Vec::new(),
        cpf: None,
        access: Access::of_system_profile(DRIVER).expect("a system profile"),
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn use_case(
    driver: Option<User>,
) -> VehicleAssignmentUseCase<MemoryAssignmentStore, MemoryUserStore> {
    use_case_with(driver, Vec::new())
}

fn use_case_with(
    driver: Option<User>,
    assignments: Vec<VehicleAssignment>,
) -> VehicleAssignmentUseCase<MemoryAssignmentStore, MemoryUserStore> {
    let users = MemoryUserStore {
        users: Mutex::new(driver.into_iter().collect()),
    };
    VehicleAssignmentUseCase::new(
        MemoryAssignmentStore {
            assignments: Mutex::new(assignments),
        },
        users,
    )
}

fn assignment(vehicle_id: i64, driver_id: i64, ended: bool) -> VehicleAssignment {
    VehicleAssignment {
        id: Some(vehicle_id),
        uuid: Some(format!("assignment-{vehicle_id}")),
        tenant_id: Some(42),
        vehicle_id,
        driver_id,
        started_at: chrono::Utc::now(),
        ended_at: ended.then(chrono::Utc::now),
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn assignment_uses_store_ports_and_returns_the_driver() {
    let use_case = use_case(Some(driver()));

    let (assignment, assigned_driver) = use_case
        .assign(&vehicle(Some(10), Some(42)), "driver-5".into())
        .await
        .expect("active same-tenant driver is assigned");

    assert_eq!(assignment.vehicle_id, 10);
    assert_eq!(assignment.driver_id, 5);
    assert_eq!(assigned_driver.uuid.as_deref(), Some("driver-5"));
}

#[tokio::test]
async fn a_missing_vehicle_id_is_rejected_instead_of_becoming_zero() {
    let use_case = use_case(Some(driver()));

    let error = use_case
        .assign(&vehicle(None, Some(42)), "driver-5".into())
        .await
        .expect_err("persisted vehicle identity is required");

    assert_eq!(error.message, "Vehicle identity is incomplete");
}

#[tokio::test]
async fn a_second_live_assignment_maps_the_store_conflict_to_the_domain_error() {
    let use_case = use_case(Some(driver()));
    use_case
        .assign(&vehicle(Some(10), Some(42)), "driver-5".into())
        .await
        .expect("first assignment succeeds");

    let error = use_case
        .assign(&vehicle(Some(10), Some(42)), "driver-5".into())
        .await
        .expect_err("vehicle already has a live assignment");

    assert_eq!(error.message, VEHICLE_ALREADY_ASSIGNED);
}

#[tokio::test]
async fn a_user_who_is_not_a_driver_is_refused() {
    let mut not_driver = driver();
    not_driver.access = Access::of_system_profile(TENANT_USER).expect("a system profile");
    let use_case = use_case(Some(not_driver));

    let error = use_case
        .assign(&vehicle(Some(10), Some(42)), "driver-5".into())
        .await
        .expect_err("only drivers can be assigned");

    assert_eq!(error.message, NOT_A_DRIVER);
}

#[tokio::test]
async fn an_unknown_driver_is_refused() {
    let use_case = use_case(None);

    let error = use_case
        .assign(&vehicle(Some(10), Some(42)), "missing-driver".into())
        .await
        .expect_err("the named driver must exist");

    assert_eq!(error.message, NOT_A_DRIVER);
}

#[tokio::test]
async fn a_disabled_or_cross_tenant_driver_is_refused() {
    let mut disabled = driver();
    disabled.enabled = false;
    let error = use_case(Some(disabled))
        .assign(&vehicle(Some(10), Some(42)), "driver-5".into())
        .await
        .expect_err("a disabled driver cannot be assigned");
    assert_eq!(error.message, NOT_A_DRIVER);

    let mut other_tenant = driver();
    other_tenant.tenant_id = Some(99);
    let error = use_case(Some(other_tenant))
        .assign(&vehicle(Some(10), Some(42)), "driver-5".into())
        .await
        .expect_err("a driver from another tenant cannot be assigned");
    assert_eq!(error.message, NOT_A_DRIVER);
}

#[tokio::test]
async fn a_driver_without_a_persisted_id_is_refused() {
    let mut no_id = driver();
    no_id.id = None;
    let error = use_case(Some(no_id))
        .assign(&vehicle(Some(10), Some(42)), "driver-5".into())
        .await
        .expect_err("the driver must have a persisted identity");

    assert_eq!(error.message, NOT_A_DRIVER);
}

#[tokio::test]
async fn current_returns_the_live_assignment_and_its_driver() {
    let use_case = use_case_with(Some(driver()), vec![assignment(10, 5, false)]);

    let (live, assigned_driver) = use_case
        .current(&vehicle(Some(10), Some(42)))
        .await
        .expect("the current assignment and driver are returned");

    assert!(live.is_live());
    assert_eq!(live.vehicle_id, 10);
    assert_eq!(assigned_driver.id, Some(5));
}

#[tokio::test]
async fn current_reports_when_the_vehicle_has_no_live_assignment() {
    let use_case = use_case_with(Some(driver()), vec![assignment(10, 5, true)]);

    let error = use_case
        .current(&vehicle(Some(10), Some(42)))
        .await
        .expect_err("ended assignments are not current");

    assert_eq!(
        error.message,
        business::use_cases::vehicle_assignment_use_case::NO_LIVE_ASSIGNMENT
    );
}

#[tokio::test]
async fn ending_a_live_assignment_preserves_it_and_returns_its_driver() {
    let use_case = use_case_with(Some(driver()), vec![assignment(10, 5, false)]);

    let (ended, assigned_driver) = use_case
        .end(&vehicle(Some(10), Some(42)))
        .await
        .expect("the live assignment is ended");

    assert!(!ended.is_live());
    assert_eq!(assigned_driver.id, Some(5));
}

#[tokio::test]
async fn history_returns_each_assignment_with_its_driver_and_total() {
    let use_case = use_case_with(
        Some(driver()),
        vec![assignment(10, 5, true), assignment(10, 5, false)],
    );

    let (history, total) = use_case
        .history(&vehicle(Some(10), Some(42)), 0, 10)
        .await
        .expect("assignment history is joined to its drivers");

    assert_eq!(total, 2);
    assert_eq!(history.len(), 2);
    assert!(history.iter().all(|(_, user)| user.id == Some(5)));
}

#[tokio::test]
async fn current_and_history_report_when_an_assigned_driver_is_missing() {
    let assignments = vec![assignment(10, 404, false)];
    let use_case = use_case_with(None, assignments);

    assert_eq!(
        use_case
            .current(&vehicle(Some(10), Some(42)))
            .await
            .expect_err("missing driver identity must not be hidden")
            .message,
        "Assigned driver not found"
    );
    assert_eq!(
        use_case
            .history(&vehicle(Some(10), Some(42)), 0, 10)
            .await
            .expect_err("history must identify missing driver references")
            .message,
        "Assigned driver not found"
    );
    assert_eq!(
        use_case
            .current(&vehicle(Some(0), Some(42)))
            .await
            .expect_err("vehicle IDs must be positive")
            .message,
        "Vehicle identity is incomplete"
    );
}
