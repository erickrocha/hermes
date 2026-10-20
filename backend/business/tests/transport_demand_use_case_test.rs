use business::domain::business_error::BusinessError;
use business::domain::enums::{Role, VehicleStatus};
use business::domain::transport_demand::TransportDemand;
use business::domain::user::User;
use business::domain::vehicle::Vehicle;
use business::gateway::transport_demand_gateway::TransportDemandStore;
use business::gateway::user_gateway::UserLookupStore;
use business::gateway::vehicle_gateway::VehicleLookupStore;
use business::use_cases::transport_demand_use_case::{NOT_A_DRIVER, TransportDemandUseCase};
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

fn driver(tenant_id: Option<i64>, role: Role) -> User {
    User {
        id: Some(7),
        uuid: Some("driver-7".into()),
        email: "driver@example.com".into(),
        name: Some("Driver".into()),
        password: "hash".into(),
        enabled: true,
        tenant_id,
        role,
        cpf: None,
        principal_function: None,
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
    let created = use_case(Some(driver(Some(42), Role::Driver)), Some(vehicle(42)))
        .create(demand)
        .await
        .expect("same-tenant driver and vehicle are accepted");

    assert_eq!(created.specific_driver_id, Some(7));
    assert_eq!(created.specific_vehicle_id, Some(10));

    let mut invalid = new_demand();
    invalid.specific_driver_id = Some(7);
    let error = use_case(Some(driver(Some(99), Role::Driver)), None)
        .create(invalid)
        .await
        .expect_err("cross-tenant driver is refused");
    assert_eq!(error.message, NOT_A_DRIVER);
}
