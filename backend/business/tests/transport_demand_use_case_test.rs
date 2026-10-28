use business::domain::business_error::BusinessError;
use business::domain::enums::{DemandKind, VehicleStatus};
use business::domain::transport_demand::TransportDemand;
use business::domain::user::{Access, User};
use business::domain::permission::{DRIVER};
use business::domain::vehicle::Vehicle;
use business::gateway::transport_demand_gateway::TransportDemandStore;
use business::gateway::user_gateway::UserLookupStore;
use business::gateway::vehicle_gateway::VehicleLookupStore;
use business::use_cases::transport_demand_use_case::{
    DAYS_OF_WEEK_UNREADABLE, KIND_NEEDS_ITS_DAYS, NOT_A_DRIVER, NOT_A_TENANT_VEHICLE,
    TransportDemandUseCase,
};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryDemandStore {
    demands: Mutex<Vec<TransportDemand>>,
}

#[async_trait]
impl TransportDemandStore for MemoryDemandStore {
    async fn save(&self, mut demand: TransportDemand) -> Result<TransportDemand, BusinessError> {
        let mut demands = self
            .demands
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let id = demand.id.unwrap_or(demands.len() as i64 + 1);
        demand.id = Some(id);
        demand.uuid = Some(format!("demand-{id}"));
        demands.push(demand.clone());
        Ok(demand)
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<TransportDemand>, BusinessError> {
        let demands = self
            .demands
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(demands.iter().find(|demand| demand.id == Some(id)).cloned())
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<TransportDemand>, BusinessError> {
        let demands = self
            .demands
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(demands
            .iter()
            .find(|demand| demand.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<TransportDemand>, u64), BusinessError> {
        let demands = self
            .demands
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let filtered: Vec<_> = demands
            .iter()
            .filter(|demand| search.is_none_or(|term| demand.demand_type.contains(term)))
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

fn new_demand() -> TransportDemand {
    TransportDemand {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        demand_type: "  Private line  ".into(),
        demand_kind: None,
        customer_id: None,
        line_name: None,
        shift_start: None,
        shift_end: None,
        days_of_week: None,
        specific_date: None,
        priority: None,
        preferred_vehicle_type: None,
        preferred_vehicle_model: None,
        specific_driver_id: None,
        specific_vehicle_id: None,
        active: true,
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

fn use_case(driver: Option<User>, vehicle: Option<Vehicle>) -> TransportDemandUseCase {
    TransportDemandUseCase::new(
        MemoryDemandStore::default(),
        MemoryUserStore { user: driver },
        MemoryVehicleStore { vehicle },
    )
}

#[tokio::test]
async fn demand_use_case_creates_a_normalized_domain_record() {
    let created = use_case(None, None)
        .create(new_demand())
        .await
        .expect("valid demand is stored");

    assert_eq!(created.demand_type, "Private line");
    assert_eq!(created.id, Some(1));
}

#[tokio::test]
async fn demand_use_case_validates_named_driver_and_vehicle_through_ports() {
    let mut demand = new_demand();
    demand.specific_driver_id = Some(7);
    demand.specific_vehicle_id = Some(10);
    let created = use_case(Some(driver(Some(42), DRIVER)), Some(vehicle(42)))
        .create(demand)
        .await
        .expect("same-tenant driver and vehicle are accepted");

    assert_eq!(created.specific_driver_id, Some(7));
    assert_eq!(created.specific_vehicle_id, Some(10));

    let mut invalid = new_demand();
    invalid.specific_driver_id = Some(7);
    let error = use_case(Some(driver(Some(99), DRIVER)), None)
        .create(invalid)
        .await
        .expect_err("cross-tenant driver is refused");
    assert_eq!(error.message, NOT_A_DRIVER);
}

#[tokio::test]
async fn demand_use_case_requires_a_type_and_valid_days_for_recurring_demand() {
    let mut blank = new_demand();
    blank.demand_type = "  ".into();
    let error = use_case(None, None)
        .create(blank)
        .await
        .expect_err("demand type is required");
    assert_eq!(error.message, "Demand type is required");

    let mut unreadable = new_demand();
    unreadable.days_of_week = Some("not-a-weekday".into());
    let error = use_case(None, None)
        .create(unreadable)
        .await
        .expect_err("weekday syntax is validated");
    assert_eq!(error.message, DAYS_OF_WEEK_UNREADABLE);

    let mut line_without_days = new_demand();
    line_without_days.demand_kind = Some(DemandKind::Line);
    let error = use_case(None, None)
        .create(line_without_days)
        .await
        .expect_err("recurring lines need a weekday");
    assert_eq!(error.message, KIND_NEEDS_ITS_DAYS);

    let mut trip_without_date = new_demand();
    trip_without_date.demand_kind = Some(DemandKind::OneOffTrip);
    let error = use_case(None, None)
        .create(trip_without_date)
        .await
        .expect_err("one-off trips need a date");
    assert_eq!(error.message, KIND_NEEDS_ITS_DAYS);
}

#[tokio::test]
async fn demand_use_case_accepts_valid_recurring_days_and_one_off_date() {
    let mut line = new_demand();
    line.demand_kind = Some(DemandKind::Line);
    line.days_of_week = Some("Mon, Wed, Fri".into());
    assert!(
        use_case(None, None)
            .create(line)
            .await
            .is_ok_and(|saved| saved.days_of_week.is_some())
    );

    let mut trip = new_demand();
    trip.demand_kind = Some(DemandKind::OneOffTrip);
    trip.specific_date = Some(chrono::NaiveDate::from_ymd_opt(2026, 10, 21).unwrap());
    assert!(use_case(None, None).create(trip).await.is_ok());
}

#[tokio::test]
async fn demand_use_case_rejects_disabled_drivers_and_cross_tenant_vehicles() {
    let mut disabled = driver(Some(42), DRIVER);
    disabled.enabled = false;
    let mut demand = new_demand();
    demand.specific_driver_id = Some(7);
    let error = use_case(Some(disabled), None)
        .create(demand)
        .await
        .expect_err("disabled driver cannot be a preference");
    assert_eq!(error.message, NOT_A_DRIVER);

    let mut demand = new_demand();
    demand.specific_vehicle_id = Some(10);
    let error = use_case(None, Some(vehicle(99)))
        .create(demand)
        .await
        .expect_err("vehicle from another tenant cannot be a preference");
    assert_eq!(error.message, NOT_A_TENANT_VEHICLE);
}

#[tokio::test]
async fn updating_a_demand_keeps_its_identity_and_lookup_reports_unknown_ids() {
    let mut existing = new_demand();
    existing.id = Some(9);
    existing.uuid = Some("demand-9".into());
    let use_case = TransportDemandUseCase::new(
        MemoryDemandStore {
            demands: Mutex::new(vec![existing]),
        },
        MemoryUserStore { user: None },
        MemoryVehicleStore { vehicle: None },
    );

    let mut changes = new_demand();
    changes.demand_type = " Updated ".into();
    let updated = use_case
        .update(9, changes)
        .await
        .expect("existing demand can be updated");
    assert_eq!(updated.id, Some(9));
    assert_eq!(updated.uuid.as_deref(), Some("demand-9"));
    assert_eq!(updated.demand_type, "Updated");

    assert_eq!(
        use_case
            .find_by_id(404)
            .await
            .expect_err("unknown id is reported")
            .message,
        "Transport demand not found"
    );
    assert_eq!(
        use_case
            .find_by_uuid("missing".into())
            .await
            .expect_err("unknown UUID is reported")
            .message,
        "Transport demand not found"
    );
    assert!(use_case.find_page(0, 10, None).await.is_ok());
}
