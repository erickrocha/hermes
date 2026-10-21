use business::domain::business_error::BusinessError;
use business::domain::enums::{
    PreventiveControlType, VehicleStatus, WorkOrderOrigin, WorkOrderStatus,
};
use business::domain::preventive_plan::PreventivePlan;
use business::domain::preventive_plan_extension::PreventivePlanExtension;
use business::domain::tenant_rule_setting::RuleSettings;
use business::domain::vehicle::Vehicle;
use business::domain::work_order::WorkOrder;
use business::domain::work_order_item::WorkOrderItem;
use business::gateway::preventive_plan_gateway::PreventivePlanStore;
use business::use_cases::preventive_plan_use_case::{
    DUPLICATE_PLAN, EXTENSION_DESCRIPTION_REQUIRED, EXTENSION_GRANT_INVALID,
    EXTENSION_ITEM_NOT_FOUND, INTERVAL_REQUIRED, ORIGIN_ORDER_NOT_ON_VEHICLE, PLAN_NAME_REQUIRED,
    PREVENTIVE_PLAN_NOT_FOUND, PreventivePlanUseCase, VEHICLE_NOT_FOUND,
};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct MemoryPreventivePlanStore {
    calls: Arc<AtomicUsize>,
    plan: Option<PreventivePlan>,
    vehicle: Option<Vehicle>,
    duplicate_plan: Option<PreventivePlan>,
    work_order: Option<WorkOrder>,
    active_work_orders: Vec<WorkOrder>,
    work_order_item: Option<WorkOrderItem>,
}

impl MemoryPreventivePlanStore {
    fn new(calls: Arc<AtomicUsize>) -> Self {
        Self {
            calls,
            plan: None,
            vehicle: None,
            duplicate_plan: None,
            work_order: None,
            active_work_orders: Vec::new(),
            work_order_item: None,
        }
    }

    fn touch(&self) {
        self.calls.fetch_add(1, Ordering::Relaxed);
    }
}

#[async_trait]
impl PreventivePlanStore for MemoryPreventivePlanStore {
    async fn save_plan(&self, plan: PreventivePlan) -> Result<PreventivePlan, BusinessError> {
        self.touch();
        Ok(plan)
    }

    async fn find_plan(&self, uuid: String) -> Result<Option<PreventivePlan>, BusinessError> {
        self.touch();
        Ok(self
            .plan
            .as_ref()
            .filter(|plan| plan.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }

    async fn find_plan_by_vehicle_and_name(
        &self,
        vehicle_id: i64,
        name: &str,
    ) -> Result<Option<PreventivePlan>, BusinessError> {
        self.touch();
        Ok(self
            .duplicate_plan
            .as_ref()
            .filter(|plan| plan.vehicle_id == vehicle_id && plan.plan_name == name)
            .cloned())
    }

    async fn list_plans(
        &self,
        _page: u64,
        _page_size: u64,
    ) -> Result<(Vec<PreventivePlan>, u64), BusinessError> {
        self.touch();
        Ok((
            self.plan.iter().cloned().collect(),
            u64::from(self.plan.is_some()),
        ))
    }

    async fn find_vehicle(&self, id: i64) -> Result<Option<Vehicle>, BusinessError> {
        self.touch();
        Ok(self
            .vehicle
            .as_ref()
            .filter(|vehicle| vehicle.id == Some(id))
            .cloned())
    }

    async fn find_work_order(&self, id: i64) -> Result<Option<WorkOrder>, BusinessError> {
        self.touch();
        Ok(self
            .work_order
            .as_ref()
            .filter(|order| order.id == Some(id))
            .cloned())
    }

    async fn active_work_orders(&self, _plan_id: i64) -> Result<Vec<WorkOrder>, BusinessError> {
        self.touch();
        Ok(self.active_work_orders.clone())
    }

    async fn save_work_order(&self, order: WorkOrder) -> Result<WorkOrder, BusinessError> {
        self.touch();
        Ok(order)
    }

    async fn find_work_order_item(
        &self,
        uuid: String,
    ) -> Result<Option<WorkOrderItem>, BusinessError> {
        self.touch();
        Ok(self
            .work_order_item
            .as_ref()
            .filter(|item| item.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }

    async fn save_work_order_item(
        &self,
        item: WorkOrderItem,
    ) -> Result<WorkOrderItem, BusinessError> {
        self.touch();
        Ok(item)
    }

    async fn save_extension(
        &self,
        extension: PreventivePlanExtension,
    ) -> Result<PreventivePlanExtension, BusinessError> {
        self.touch();
        Ok(extension)
    }

    async fn rules(&self, _tenant_id: Option<i64>) -> Result<RuleSettings, BusinessError> {
        self.touch();
        Ok(RuleSettings::default())
    }
}

fn plan(name: &str) -> PreventivePlan {
    PreventivePlan {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        vehicle_id: 10,
        plan_name: name.into(),
        control_type: PreventiveControlType::Kilometers,
        interval_km: Some(10_000.0),
        interval_days: None,
        last_service_km: Some(50_000.0),
        last_service_date: None,
        extension_limit_km: None,
        last_work_order_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn paginated_plans_recompute_status_and_missing_plan_lookup_is_explicit() {
    let mut saved_plan = plan("Oil");
    saved_plan.uuid = Some("plan-12".into());
    let mut store = MemoryPreventivePlanStore::new(Arc::new(AtomicUsize::new(0)));
    store.plan = Some(saved_plan.clone());
    store.vehicle = Some(vehicle(Some(60_000.0)));
    let use_case = PreventivePlanUseCase::new(store);
    let (plans, total) = use_case.find_page(0, 10).await.unwrap();
    assert_eq!(total, 1);
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].0, saved_plan);
    let (_, status) = use_case.find_by_uuid("plan-12".into()).await.unwrap();
    assert_eq!(plans[0].1, status);
    assert_eq!(
        use_case
            .find_by_uuid("missing".into())
            .await
            .unwrap_err()
            .message,
        PREVENTIVE_PLAN_NOT_FOUND
    );

    let empty = PreventivePlanUseCase::new(MemoryPreventivePlanStore::new(Arc::new(
        AtomicUsize::new(0),
    )));
    assert_eq!(empty.find_page(0, 10).await.unwrap().1, 0);
}
fn vehicle(odometer_km: Option<f64>) -> Vehicle {
    Vehicle {
        id: Some(10),
        uuid: Some("vehicle-10".into()),
        tenant_id: Some(42),
        plate: "ABC1234".into(),
        model: "Truck".into(),
        status: VehicleStatus::Active,
        tracker_device_id: None,
        prefix: None,
        vehicle_type: None,
        odometer_km,
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

fn work_order(vehicle_id: i64, status: WorkOrderStatus) -> WorkOrder {
    WorkOrder {
        id: Some(30),
        uuid: Some("order-30".into()),
        tenant_id: Some(42),
        vehicle_id,
        opened_at: None,
        odometer_km: 40_000.0,
        origin: WorkOrderOrigin::Manual,
        checklist_run_id: None,
        maintenance_plan_id: None,
        preventive_plan_id: None,
        service_type: None,
        description: "Inspection".into(),
        responsible: None,
        status,
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

fn work_order_item(status: business::domain::enums::WorkOrderItemStatus) -> WorkOrderItem {
    WorkOrderItem {
        id: Some(35),
        uuid: Some("item-35".into()),
        tenant_id: Some(42),
        work_order_id: 30,
        description: "Oil service".into(),
        item_type: None,
        status,
        observation: None,
        resolved_by: None,
        resolved_at: None,
        resolution_description: None,
        purchase_order_id: None,
        is_purchase_placeholder: false,
        preventive_plan_id: Some(12),
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn a_blank_plan_name_is_rejected_before_any_store_call() {
    let calls = Arc::new(AtomicUsize::new(0));
    let use_case = PreventivePlanUseCase::new(MemoryPreventivePlanStore::new(Arc::clone(&calls)));

    let error = use_case
        .create(plan("  "))
        .await
        .expect_err("plan name is required");

    assert_eq!(error.message, PLAN_NAME_REQUIRED);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn plan_creation_validates_intervals_vehicle_tenant_and_duplicate_name() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut store = MemoryPreventivePlanStore::new(Arc::clone(&calls));
    store.vehicle = Some(vehicle(Some(60_000.0)));
    let use_case = PreventivePlanUseCase::new(store);

    let mut invalid = plan("Oil");
    invalid.interval_km = None;
    assert_eq!(
        use_case
            .create(invalid)
            .await
            .expect_err("kilometre plan needs an interval")
            .message,
        INTERVAL_REQUIRED
    );
    assert_eq!(
        use_case
            .create(plan("Oil"))
            .await
            .expect("valid plan is saved")
            .plan_name,
        "Oil"
    );

    let missing_vehicle = PreventivePlanUseCase::new(MemoryPreventivePlanStore::new(Arc::new(
        AtomicUsize::new(0),
    )));
    assert_eq!(
        missing_vehicle
            .create(plan("Oil"))
            .await
            .expect_err("vehicle is required")
            .message,
        VEHICLE_NOT_FOUND
    );

    let mut wrong_tenant_store = MemoryPreventivePlanStore::new(Arc::new(AtomicUsize::new(0)));
    let mut other_tenant_vehicle = vehicle(Some(60_000.0));
    other_tenant_vehicle.tenant_id = Some(99);
    wrong_tenant_store.vehicle = Some(other_tenant_vehicle);
    assert_eq!(
        PreventivePlanUseCase::new(wrong_tenant_store)
            .create(plan("Oil"))
            .await
            .expect_err("vehicle must belong to the plan tenant")
            .message,
        VEHICLE_NOT_FOUND
    );

    let mut duplicate_store = MemoryPreventivePlanStore::new(Arc::new(AtomicUsize::new(0)));
    duplicate_store.vehicle = Some(vehicle(Some(60_000.0)));
    duplicate_store.duplicate_plan = Some(plan("Oil"));
    assert_eq!(
        PreventivePlanUseCase::new(duplicate_store)
            .create(plan("Oil"))
            .await
            .expect_err("plan name is unique per vehicle")
            .message,
        DUPLICATE_PLAN
    );
}

#[tokio::test]
async fn due_plans_generate_one_preventive_order_or_return_the_active_one() {
    let mut due_plan = plan("Oil");
    due_plan.id = Some(12);
    due_plan.uuid = Some("plan-12".into());
    let mut store = MemoryPreventivePlanStore::new(Arc::new(AtomicUsize::new(0)));
    store.plan = Some(due_plan.clone());
    store.vehicle = Some(vehicle(Some(60_000.0)));
    let use_case = PreventivePlanUseCase::new(store);

    let (generated, created) = use_case
        .generate_work_order("plan-12".into())
        .await
        .expect("overdue plan can open an order");
    assert!(created);
    assert_eq!(generated.origin, WorkOrderOrigin::Preventive);
    assert_eq!(generated.preventive_plan_id, Some(12));
    assert_eq!(generated.odometer_km, 60_000.0);

    let mut store = MemoryPreventivePlanStore::new(Arc::new(AtomicUsize::new(0)));
    store.plan = Some(due_plan);
    store.vehicle = Some(vehicle(Some(60_000.0)));
    store.active_work_orders = vec![work_order(10, WorkOrderStatus::Open)];
    let use_case = PreventivePlanUseCase::new(store);
    let (existing, created) = use_case
        .generate_work_order("plan-12".into())
        .await
        .expect("existing active order is reused");
    assert!(!created);
    assert_eq!(existing.id, Some(30));
}

#[tokio::test]
async fn extending_a_plan_resolves_the_pending_item_and_uses_current_odometer() {
    use business::domain::enums::WorkOrderItemStatus;

    let mut preventive_plan = plan("Oil");
    preventive_plan.id = Some(12);
    preventive_plan.uuid = Some("plan-12".into());
    let calls = Arc::new(AtomicUsize::new(0));
    let mut store = MemoryPreventivePlanStore::new(Arc::clone(&calls));
    store.plan = Some(preventive_plan);
    store.vehicle = Some(vehicle(Some(61_000.0)));
    store.work_order = Some(work_order(10, WorkOrderStatus::Open));
    store.work_order_item = Some(work_order_item(WorkOrderItemStatus::Pending));

    let extension = PreventivePlanUseCase::new(store)
        .extend(
            "plan-12".into(),
            "item-35".into(),
            None,
            5_000.0,
            "  inspected, no replacement  ".into(),
        )
        .await
        .expect("eligible pending item can extend the plan");

    assert_eq!(extension.inspection_km, 61_000.0);
    assert_eq!(extension.granted_km, 5_000.0);
    assert_eq!(extension.resulting_limit_km, 66_000.0);
    assert_eq!(extension.description, "inspected, no replacement");
    assert_eq!(calls.load(Ordering::Relaxed), 11);
}

#[tokio::test]
async fn plan_extension_rejects_empty_description_invalid_grant_and_missing_item() {
    let mut preventive_plan = plan("Oil");
    preventive_plan.id = Some(12);
    preventive_plan.uuid = Some("plan-12".into());
    let calls = Arc::new(AtomicUsize::new(0));
    let use_case = PreventivePlanUseCase::new(MemoryPreventivePlanStore::new(Arc::clone(&calls)));
    assert_eq!(
        use_case
            .extend(
                "plan-12".into(),
                "item-35".into(),
                None,
                1_000.0,
                " ".into(),
            )
            .await
            .expect_err("description is required")
            .message,
        EXTENSION_DESCRIPTION_REQUIRED
    );
    assert_eq!(calls.load(Ordering::Relaxed), 0);

    let mut store = MemoryPreventivePlanStore::new(Arc::new(AtomicUsize::new(0)));
    store.plan = Some(preventive_plan.clone());
    assert_eq!(
        PreventivePlanUseCase::new(store)
            .extend(
                "plan-12".into(),
                "item-35".into(),
                None,
                f64::NAN,
                "inspection".into(),
            )
            .await
            .expect_err("grant must be positive")
            .message,
        EXTENSION_GRANT_INVALID
    );

    let mut store = MemoryPreventivePlanStore::new(Arc::new(AtomicUsize::new(0)));
    store.plan = Some(preventive_plan);
    assert_eq!(
        PreventivePlanUseCase::new(store)
            .extend(
                "plan-12".into(),
                "missing".into(),
                None,
                1_000.0,
                "inspection".into(),
            )
            .await
            .expect_err("item must exist")
            .message,
        EXTENSION_ITEM_NOT_FOUND
    );
}

#[tokio::test]
async fn plan_updates_reset_extensions_only_for_a_valid_new_service_cycle() {
    let mut current = plan("Oil");
    current.id = Some(12);
    current.uuid = Some("plan-12".into());
    current.last_work_order_id = Some(20);
    current.extension_limit_km = Some(66_000.0);

    let mut store = MemoryPreventivePlanStore::new(Arc::new(AtomicUsize::new(0)));
    store.plan = Some(current.clone());
    store.work_order = Some(work_order(10, WorkOrderStatus::Open));
    let use_case = PreventivePlanUseCase::new(store);
    let mut edited = current.clone();
    edited.last_service_km = Some(61_000.0);
    edited.last_work_order_id = Some(30);
    let updated = use_case
        .update(edited)
        .await
        .expect("new cycle with an order for the same vehicle is accepted");
    assert_eq!(updated.last_work_order_id, Some(30));
    assert_eq!(updated.extension_limit_km, None);

    let mut store = MemoryPreventivePlanStore::new(Arc::new(AtomicUsize::new(0)));
    store.plan = Some(current.clone());
    let use_case = PreventivePlanUseCase::new(store);
    let mut edited = current.clone();
    edited.last_work_order_id = Some(99);
    let unchanged_cycle = use_case
        .update(edited)
        .await
        .expect("order reference is ignored when the service base is unchanged");
    assert_eq!(unchanged_cycle.last_work_order_id, Some(20));
    assert_eq!(unchanged_cycle.extension_limit_km, Some(66_000.0));

    let mut store = MemoryPreventivePlanStore::new(Arc::new(AtomicUsize::new(0)));
    store.plan = Some(current);
    store.work_order = Some(work_order(99, WorkOrderStatus::Open));
    let use_case = PreventivePlanUseCase::new(store);
    let mut edited = plan("Oil");
    edited.uuid = Some("plan-12".into());
    edited.last_service_km = Some(61_000.0);
    edited.last_work_order_id = Some(30);
    assert_eq!(
        use_case
            .update(edited)
            .await
            .expect_err("origin order must use the plan vehicle")
            .message,
        ORIGIN_ORDER_NOT_ON_VEHICLE
    );
}
