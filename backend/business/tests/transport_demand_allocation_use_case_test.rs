use business::domain::business_error::BusinessError;
use business::domain::enums::{VehicleStatus};
use business::domain::transport_demand_allocation::TransportDemandAllocation;
use business::domain::user::{Access, User};
use business::domain::permission::{DRIVER, TENANT_USER};
use business::domain::vehicle::Vehicle;
use business::gateway::transport_demand_allocation_gateway::TransportDemandAllocationStore;
use business::gateway::user_gateway::UserLookupStore;
use business::gateway::vehicle_gateway::VehicleLookupStore;
use business::use_cases::transport_demand_allocation_use_case::{
    ENDS_BEFORE_IT_STARTS, NOT_A_DRIVER, NOT_A_TENANT_VEHICLE, TransportDemandAllocationUseCase,
};
use chrono::NaiveDate;
use sea_orm::prelude::async_trait::async_trait;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct MemoryAllocationStore(Arc<Mutex<Vec<TransportDemandAllocation>>>);

#[async_trait]
impl TransportDemandAllocationStore for MemoryAllocationStore {
    async fn save(
        &self,
        mut allocation: TransportDemandAllocation,
    ) -> Result<TransportDemandAllocation, BusinessError> {
        let mut allocations = self
            .0
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let id = allocation.id.unwrap_or(allocations.len() as i64 + 1);
        allocation.id = Some(id);
        allocation.uuid = Some(format!("allocation-{id}"));
        allocations.push(allocation.clone());
        Ok(allocation)
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<TransportDemandAllocation>, BusinessError> {
        let allocations = self
            .0
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(allocations
            .iter()
            .find(|allocation| allocation.id == Some(id))
            .cloned())
    }

    async fn get_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<TransportDemandAllocation>, BusinessError> {
        let allocations = self
            .0
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(allocations
            .iter()
            .find(|allocation| allocation.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }

    async fn list_by_demand(
        &self,
        demand_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<TransportDemandAllocation>, u64), BusinessError> {
        let allocations = self
            .0
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let filtered: Vec<_> = allocations
            .iter()
            .filter(|allocation| allocation.demand_id == demand_id)
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

fn allocation() -> TransportDemandAllocation {
    TransportDemandAllocation {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        demand_id: 5,
        driver_id: 7,
        vehicle_id: 10,
        days_of_week: Some("Mon,Wed".into()),
        start_date: NaiveDate::from_ymd_opt(2026, 10, 1).expect("valid test date"),
        end_date: None,
        active: true,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn driver(tenant_id: Option<i64>, profile_key: &str) -> User {
    User {
        id: Some(7),
        uuid: Some("driver-7".into()),
        email: "driver@example.com".into(),
        name: Some("Driver".into()),
        password: "hash".into(),
        enabled: true,
        tenant_id,
        profile_id: Some(1),
        extra_permissions: Vec::new(),
        cpf: None,
        access: Access::of_system_profile(profile_key).expect("a system profile"),
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn vehicle(tenant_id: i64) -> Vehicle {
    Vehicle {
        id: Some(10),
        uuid: Some("vehicle-10".into()),
        tenant_id: Some(tenant_id),
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

fn use_case(user: Option<User>, vehicle: Option<Vehicle>) -> TransportDemandAllocationUseCase {
    use_case_with_store(MemoryAllocationStore::default(), user, vehicle)
}

fn use_case_with_store(
    store: MemoryAllocationStore,
    user: Option<User>,
    vehicle: Option<Vehicle>,
) -> TransportDemandAllocationUseCase {
    TransportDemandAllocationUseCase::new(
        store,
        MemoryUserStore { user },
        MemoryVehicleStore { vehicle },
    )
}

#[tokio::test]
async fn allocation_validates_and_stores_a_same_tenant_driver_and_vehicle() {
    let created = use_case(Some(driver(Some(42), DRIVER)), Some(vehicle(42)))
        .create(allocation())
        .await
        .expect("valid allocation is stored");

    assert_eq!(created.id, Some(1));
    assert_eq!(created.driver_id, 7);
    assert_eq!(created.vehicle_id, 10);
}

#[tokio::test]
async fn allocation_rejects_a_user_who_is_not_an_active_same_tenant_driver() {
    let error = use_case(Some(driver(Some(99), TENANT_USER)), Some(vehicle(42)))
        .create(allocation())
        .await
        .expect_err("invalid driver is refused");

    assert_eq!(error.message, NOT_A_DRIVER);
}

#[tokio::test]
async fn allocation_rejects_a_vehicle_from_another_tenant() {
    let error = use_case(Some(driver(Some(42), DRIVER)), Some(vehicle(99)))
        .create(allocation())
        .await
        .expect_err("cross-tenant vehicle is refused");

    assert_eq!(error.message, NOT_A_TENANT_VEHICLE);
}

#[tokio::test]
async fn allocation_rejects_an_end_date_before_its_start_date_and_accepts_the_boundary_and_open_ended()
 {
    let uc = use_case(Some(driver(Some(42), DRIVER)), Some(vehicle(42)));

    let mut inverted = allocation();
    inverted.end_date = NaiveDate::from_ymd_opt(2026, 9, 30);
    let error = uc
        .create(inverted.clone())
        .await
        .expect_err("an allocation that ends before it starts is refused");
    assert_eq!(error.message, ENDS_BEFORE_IT_STARTS);

    let mut same_day = allocation();
    same_day.end_date = Some(same_day.start_date);
    uc.create(same_day)
        .await
        .expect("a one-day allocation is valid");

    let created = uc
        .create(allocation())
        .await
        .expect("an open-ended allocation is valid");
    let error = uc
        .update(created.id.expect("id"), inverted)
        .await
        .expect_err("an edit cannot invert the range either");
    assert_eq!(error.message, ENDS_BEFORE_IT_STARTS);
}

#[tokio::test]
async fn allocations_can_be_found_updated_and_paged() {
    let store = MemoryAllocationStore::default();
    let use_case = use_case_with_store(
        store,
        Some(driver(Some(42), DRIVER)),
        Some(vehicle(42)),
    );
    let created = use_case.create(allocation()).await.unwrap();
    let uuid = created.uuid.clone().unwrap();
    assert_eq!(use_case.find_by_id(1).await.unwrap(), created);
    assert_eq!(use_case.find_by_uuid(uuid).await.unwrap(), created);
    assert_eq!(
        use_case.history(5, 0, 10).await.unwrap(),
        (vec![created.clone()], 1)
    );

    let mut edited = allocation();
    edited.demand_id = 99;
    edited.days_of_week = Some("2".into());
    let updated = use_case.update(1, edited).await.unwrap();
    assert_eq!(updated.id, Some(1));
    assert_eq!(updated.uuid, created.uuid);
    assert_eq!(updated.demand_id, created.demand_id);
    assert_eq!(updated.tenant_id, created.tenant_id);
    assert_eq!(updated.days_of_week.as_deref(), Some("2"));
}

#[tokio::test]
async fn allocations_reject_unreadable_days_and_report_missing_rows() {
    let use_case = use_case(Some(driver(Some(42), DRIVER)), Some(vehicle(42)));
    let mut invalid = allocation();
    invalid.days_of_week = Some("Funday".into());
    assert_eq!(
        use_case
            .create(invalid)
            .await
            .expect_err("unknown weekday is rejected")
            .message,
        business::use_cases::transport_demand_allocation_use_case::DAYS_OF_WEEK_UNREADABLE
    );
    assert_eq!(
        use_case
            .find_by_id(404)
            .await
            .expect_err("unknown allocation id is not found")
            .message,
        "Allocation not found"
    );
    assert_eq!(
        use_case
            .find_by_uuid("missing".into())
            .await
            .expect_err("unknown allocation uuid is not found")
            .message,
        "Allocation not found"
    );
    assert_eq!(
        use_case
            .update(404, allocation())
            .await
            .expect_err("missing allocation cannot be updated")
            .message,
        "Allocation not found"
    );
}
