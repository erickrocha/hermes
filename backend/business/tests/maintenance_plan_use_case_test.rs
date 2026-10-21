use business::domain::business_error::BusinessError;
use business::domain::enums::{MaintenancePlanStatus, VehicleStatus};
use business::domain::maintenance_plan::MaintenancePlan;
use business::domain::vehicle::Vehicle;
use business::domain::work_order::WorkOrder;
use business::gateway::maintenance_plan_gateway::MaintenancePlanStore;
use business::gateway::vehicle_gateway::VehicleLookupStore;
use business::gateway::work_order_gateway::WorkOrderStore;
use business::use_cases::maintenance_plan_use_case::{
    MaintenancePlanUseCase, NOT_A_TENANT_VEHICLE, OVERLAPPING_PLAN_EXISTS,
};
use chrono::NaiveDate;
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryPlanStore {
    plans: Mutex<Vec<MaintenancePlan>>,
}

#[async_trait]
impl MaintenancePlanStore for MemoryPlanStore {
    async fn save(&self, mut plan: MaintenancePlan) -> Result<MaintenancePlan, BusinessError> {
        let mut plans = self
            .plans
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let id = plan.id.unwrap_or(plans.len() as i64 + 1);
        plan.id = Some(id);
        plan.uuid = Some(format!("maintenance-plan-{id}"));
        plans.push(plan.clone());
        Ok(plan)
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<MaintenancePlan>, BusinessError> {
        let plans = self
            .plans
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(plans.iter().find(|plan| plan.id == Some(id)).cloned())
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<MaintenancePlan>, BusinessError> {
        let plans = self
            .plans
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(plans
            .iter()
            .find(|plan| plan.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }

    async fn find_by_vehicle_and_date(
        &self,
        vehicle_id: i64,
        date: NaiveDate,
    ) -> Result<Vec<MaintenancePlan>, BusinessError> {
        let plans = self
            .plans
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(plans
            .iter()
            .filter(|plan| plan.vehicle_id == vehicle_id && plan.date == date)
            .cloned()
            .collect())
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<MaintenancePlan>, u64), BusinessError> {
        let plans = self
            .plans
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let total = plans.len() as u64;
        let start = (page * page_size) as usize;
        Ok((
            plans
                .iter()
                .skip(start)
                .take(page_size as usize)
                .cloned()
                .collect(),
            total,
        ))
    }
}

#[derive(Default)]
struct MemoryWorkOrderStore {
    work_order: Option<WorkOrder>,
}

#[async_trait]
impl WorkOrderStore for MemoryWorkOrderStore {
    async fn save(&self, work_order: WorkOrder) -> Result<WorkOrder, BusinessError> {
        Ok(work_order)
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<WorkOrder>, BusinessError> {
        Ok(self
            .work_order
            .as_ref()
            .filter(|work_order| work_order.id == Some(id))
            .cloned())
    }

    async fn list_by_maintenance_plan(
        &self,
        plan_id: i64,
    ) -> Result<Vec<WorkOrder>, BusinessError> {
        Ok(self
            .work_order
            .as_ref()
            .filter(|work_order| work_order.maintenance_plan_id == Some(plan_id))
            .cloned()
            .into_iter()
            .collect())
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

fn new_plan() -> MaintenancePlan {
    MaintenancePlan {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        vehicle_id: 10,
        date: NaiveDate::from_ymd_opt(2026, 10, 1).expect("valid test date"),
        planned_start: None,
        planned_end: None,
        status: MaintenancePlanStatus::Scheduled,
        affects_schedule: false,
        origin: "Manual".into(),
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn work_order(maintenance_plan_id: Option<i64>) -> WorkOrder {
    WorkOrder {
        id: Some(22),
        uuid: Some("work-order-22".into()),
        tenant_id: Some(42),
        vehicle_id: 10,
        opened_at: None,
        odometer_km: 100.0,
        origin: business::domain::enums::WorkOrderOrigin::Manual,
        checklist_run_id: None,
        maintenance_plan_id,
        preventive_plan_id: None,
        service_type: Some("Repair".into()),
        description: "Repair brakes".into(),
        responsible: None,
        status: business::domain::enums::WorkOrderStatus::Open,
        observation: None,
        external_service: false,
        supplier: None,
        invoice_number: None,
        invoice_value_cents: None,
        invoice_date: None,
        concluded_at: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn use_case(plans: MemoryPlanStore, vehicle: Vehicle) -> MaintenancePlanUseCase {
    MaintenancePlanUseCase::new(
        plans,
        MemoryWorkOrderStore::default(),
        MemoryVehicleStore {
            vehicle: Some(vehicle),
        },
    )
}

#[tokio::test]
async fn maintenance_plan_uses_ports_to_validate_and_store_a_plan() {
    let use_case = use_case(MemoryPlanStore::default(), vehicle(42));

    let (created, work_orders) = use_case
        .create(new_plan(), Vec::new())
        .await
        .expect("plan for a same-tenant vehicle is stored");

    assert_eq!(created.id, Some(1));
    assert_eq!(created.status, MaintenancePlanStatus::Scheduled);
    assert!(work_orders.is_empty());
}

#[tokio::test]
async fn maintenance_plan_refuses_a_vehicle_owned_by_another_tenant() {
    let use_case = use_case(MemoryPlanStore::default(), vehicle(99));

    let error = use_case
        .create(new_plan(), Vec::new())
        .await
        .expect_err("cross-tenant vehicle is refused");

    assert_eq!(error.message, NOT_A_TENANT_VEHICLE);
}

#[tokio::test]
async fn maintenance_plan_refuses_an_overlapping_active_plan() {
    let plans = MemoryPlanStore {
        plans: Mutex::new(vec![new_plan()]),
    };
    let use_case = use_case(plans, vehicle(42));

    let error = use_case
        .create(new_plan(), Vec::new())
        .await
        .expect_err("overlapping active plan is refused");

    assert_eq!(error.message, OVERLAPPING_PLAN_EXISTS);
}

#[tokio::test]
async fn maintenance_plan_links_a_valid_work_order_to_the_saved_plan() {
    let use_case = MaintenancePlanUseCase::new(
        MemoryPlanStore::default(),
        MemoryWorkOrderStore {
            work_order: Some(work_order(None)),
        },
        MemoryVehicleStore {
            vehicle: Some(vehicle(42)),
        },
    );

    let (plan, linked) = use_case
        .create(new_plan(), vec![22])
        .await
        .expect("same-tenant work order is linked");

    assert_eq!(plan.id, Some(1));
    assert_eq!(linked.len(), 1);
    assert_eq!(linked[0].maintenance_plan_id, plan.id);
}

#[tokio::test]
async fn maintenance_plan_rejects_a_work_order_already_in_an_active_plan() {
    let mut existing_plan = new_plan();
    existing_plan.id = Some(5);
    existing_plan.vehicle_id = 99;
    let mut assigned_order = work_order(Some(5));
    assigned_order.vehicle_id = 10;
    let use_case = MaintenancePlanUseCase::new(
        MemoryPlanStore {
            plans: Mutex::new(vec![existing_plan]),
        },
        MemoryWorkOrderStore {
            work_order: Some(assigned_order),
        },
        MemoryVehicleStore {
            vehicle: Some(vehicle(42)),
        },
    );

    let error = use_case
        .create(new_plan(), vec![22])
        .await
        .expect_err("one work order cannot be assigned to two active plans");
    assert_eq!(
        error.message,
        business::use_cases::maintenance_plan_use_case::WORK_ORDER_ALREADY_SCHEDULED
    );
}

#[tokio::test]
async fn maintenance_plan_allows_a_work_order_from_an_inactive_plan_to_be_rescheduled() {
    let mut concluded_plan = new_plan();
    concluded_plan.id = Some(5);
    concluded_plan.vehicle_id = 99;
    concluded_plan.status = MaintenancePlanStatus::Concluded;
    let mut assigned_order = work_order(Some(5));
    assigned_order.vehicle_id = 10;
    let use_case = MaintenancePlanUseCase::new(
        MemoryPlanStore {
            plans: Mutex::new(vec![concluded_plan]),
        },
        MemoryWorkOrderStore {
            work_order: Some(assigned_order),
        },
        MemoryVehicleStore {
            vehicle: Some(vehicle(42)),
        },
    );

    let (_, linked) = use_case
        .create(new_plan(), vec![22])
        .await
        .expect("a work order from a concluded plan can be rescheduled");
    assert_eq!(linked[0].maintenance_plan_id, Some(2));
}

#[tokio::test]
async fn maintenance_plan_rejects_missing_or_mismatched_work_orders() {
    let use_case = use_case(MemoryPlanStore::default(), vehicle(42));
    let error = use_case
        .create(new_plan(), vec![404])
        .await
        .expect_err("unknown work order cannot be scheduled");
    assert_eq!(
        error.message,
        business::use_cases::maintenance_plan_use_case::WORK_ORDER_NOT_FOUND
    );

    let mut outside_vehicle = work_order(None);
    outside_vehicle.vehicle_id = 99;
    outside_vehicle.tenant_id = Some(99);
    let use_case = MaintenancePlanUseCase::new(
        MemoryPlanStore::default(),
        MemoryWorkOrderStore {
            work_order: Some(outside_vehicle),
        },
        MemoryVehicleStore {
            vehicle: Some(vehicle(42)),
        },
    );
    let error = use_case
        .create(new_plan(), vec![22])
        .await
        .expect_err("work order for a different vehicle/tenant is refused");
    assert_eq!(
        error.message,
        business::use_cases::maintenance_plan_use_case::WORK_ORDER_WRONG_VEHICLE
    );
}

#[tokio::test]
async fn maintenance_plan_lookup_returns_its_work_orders_and_page() {
    let mut plan = new_plan();
    plan.id = Some(1);
    plan.uuid = Some("plan-1".into());
    let use_case = MaintenancePlanUseCase::new(
        MemoryPlanStore {
            plans: Mutex::new(vec![plan]),
        },
        MemoryWorkOrderStore {
            work_order: Some(work_order(Some(1))),
        },
        MemoryVehicleStore {
            vehicle: Some(vehicle(42)),
        },
    );

    let (found, work_orders) = use_case
        .find_by_uuid("plan-1".into())
        .await
        .expect("existing plan and work orders are returned");
    assert_eq!(found.id, Some(1));
    assert_eq!(work_orders.len(), 1);
    let (page, total) = use_case.find_page(0, 10).await.expect("plans are paged");
    assert_eq!(page.len(), 1);
    assert_eq!(total, 1);
}
