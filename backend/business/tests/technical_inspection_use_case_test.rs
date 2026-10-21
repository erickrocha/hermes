use business::domain::business_error::BusinessError;
use business::domain::enums::{
    PreventiveControlType, WorkOrderItemStatus, WorkOrderOrigin, WorkOrderStatus,
};
use business::domain::inspection_model::InspectionModel;
use business::domain::preventive_plan::PreventivePlan;
use business::domain::preventive_plan_alert::PreventivePlanAlert;
use business::domain::technical_inspection::TechnicalInspection;
use business::domain::technical_inspection_item::TechnicalInspectionItem;
use business::domain::vehicle::Vehicle;
use business::domain::work_order::WorkOrder;
use business::domain::work_order_item::WorkOrderItem;
use business::gateway::inspection_model_gateway::InspectionModelGateway;
use business::gateway::preventive_plan_alert_gateway::PreventivePlanAlertGateway;
use business::gateway::preventive_plan_gateway::PreventivePlanGateway;
use business::gateway::technical_inspection_gateway::TechnicalInspectionGateway;
use business::gateway::technical_inspection_item_gateway::TechnicalInspectionItemGateway;
use business::gateway::technical_inspection_workflow_gateway::TechnicalInspectionDatabaseGateway;
use business::gateway::technical_inspection_workflow_gateway::TechnicalInspectionStore;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::gateway::work_order_gateway::WorkOrderGateway;
use business::gateway::work_order_item_gateway::WorkOrderItemGateway;
use business::use_cases::technical_inspection_use_case::{
    AT_LEAST_ONE_ITEM_REQUIRED, INSPECTION_NOT_FOUND, ITEM_DESCRIPTION_REQUIRED, MODEL_REQUIRED,
    NOT_A_TENANT_VEHICLE, TechnicalInspectionUseCase,
};
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{DatabaseBackend, MockDatabase};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct MemoryInspectionStore {
    vehicle: Option<Vehicle>,
    model: Option<InspectionModel>,
    inspection: Option<TechnicalInspection>,
    open_orders: Vec<WorkOrder>,
    work_order_items: Vec<WorkOrderItem>,
    inspection_items: Vec<TechnicalInspectionItem>,
    plans: Vec<PreventivePlan>,
    alerts: Vec<PreventivePlanAlert>,
    saved_alerts: Vec<PreventivePlanAlert>,
    saved_orders: Vec<WorkOrder>,
}

#[derive(Clone)]
struct TestInspectionStore(Arc<Mutex<MemoryInspectionStore>>);

#[async_trait]
impl TechnicalInspectionStore for TestInspectionStore {
    async fn find_vehicle(&self, _id: i64) -> Result<Option<Vehicle>, BusinessError> {
        Ok(self.0.lock().unwrap().vehicle.clone())
    }

    async fn find_model(
        &self,
        name: &str,
        _tenant_id: Option<i64>,
    ) -> Result<Option<InspectionModel>, BusinessError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .model
            .clone()
            .filter(|model| model.name.eq_ignore_ascii_case(name)))
    }

    async fn save_inspection(
        &self,
        mut inspection: TechnicalInspection,
    ) -> Result<TechnicalInspection, BusinessError> {
        inspection.id = Some(101);
        inspection.uuid = Some("inspection-101".into());
        self.0.lock().unwrap().inspection = Some(inspection.clone());
        Ok(inspection)
    }

    async fn active_work_orders_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Vec<WorkOrder>, BusinessError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .open_orders
            .iter()
            .filter(|order| order.vehicle_id == vehicle_id && order.status == WorkOrderStatus::Open)
            .cloned()
            .collect())
    }

    async fn list_work_order_items(
        &self,
        work_order_id: i64,
    ) -> Result<Vec<WorkOrderItem>, BusinessError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .work_order_items
            .iter()
            .filter(|item| item.work_order_id == work_order_id)
            .cloned()
            .collect())
    }

    async fn save_work_order_item(
        &self,
        mut item: WorkOrderItem,
    ) -> Result<WorkOrderItem, BusinessError> {
        item.id.get_or_insert(202);
        self.0
            .lock()
            .unwrap()
            .work_order_items
            .retain(|existing| existing.id != item.id);
        self.0.lock().unwrap().work_order_items.push(item.clone());
        Ok(item)
    }

    async fn save_work_order(&self, mut order: WorkOrder) -> Result<WorkOrder, BusinessError> {
        order.id = Some(201);
        order.uuid = Some("work-order-201".into());
        self.0.lock().unwrap().saved_orders.push(order.clone());
        Ok(order)
    }

    async fn save_inspection_item(
        &self,
        mut item: TechnicalInspectionItem,
    ) -> Result<TechnicalInspectionItem, BusinessError> {
        item.id = Some(301 + self.0.lock().unwrap().inspection_items.len() as i64);
        self.0.lock().unwrap().inspection_items.push(item.clone());
        Ok(item)
    }

    async fn plans_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Vec<PreventivePlan>, BusinessError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .plans
            .iter()
            .filter(|plan| plan.vehicle_id == vehicle_id)
            .cloned()
            .collect())
    }

    async fn alerts_by_plan(
        &self,
        plan_id: i64,
    ) -> Result<Vec<PreventivePlanAlert>, BusinessError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .alerts
            .iter()
            .filter(|alert| alert.preventive_plan_id == plan_id)
            .cloned()
            .collect())
    }

    async fn save_alert(
        &self,
        alert: PreventivePlanAlert,
    ) -> Result<PreventivePlanAlert, BusinessError> {
        self.0.lock().unwrap().saved_alerts.push(alert.clone());
        Ok(alert)
    }

    async fn inspection_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<TechnicalInspection>, BusinessError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .inspection
            .clone()
            .filter(|inspection| inspection.uuid.as_deref() == Some(uuid.as_str())))
    }

    async fn list_inspection_items(
        &self,
        inspection_id: i64,
    ) -> Result<Vec<TechnicalInspectionItem>, BusinessError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .inspection_items
            .iter()
            .filter(|item| item.technical_inspection_id == inspection_id)
            .cloned()
            .collect())
    }

    async fn find_work_order_item(&self, id: i64) -> Result<Option<WorkOrderItem>, BusinessError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .work_order_items
            .iter()
            .find(|item| item.id == Some(id))
            .cloned())
    }

    async fn find_work_order(&self, id: i64) -> Result<Option<WorkOrder>, BusinessError> {
        let store = self.0.lock().unwrap();
        Ok(store
            .saved_orders
            .iter()
            .chain(store.open_orders.iter())
            .find(|order| order.id == Some(id))
            .cloned())
    }
}

fn memory_use_case(
    store: MemoryInspectionStore,
) -> (
    TechnicalInspectionUseCase,
    Arc<Mutex<MemoryInspectionStore>>,
) {
    let store = Arc::new(Mutex::new(store));
    (
        TechnicalInspectionUseCase::new(TestInspectionStore(store.clone())),
        store,
    )
}

fn memory_vehicle() -> Vehicle {
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
        odometer_km: Some(12_000.0),
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

fn answer(description: &str, conforming: bool) -> TechnicalInspectionItem {
    TechnicalInspectionItem {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        technical_inspection_id: 0,
        description: description.into(),
        conforming,
        observation: Some("Observed during inspection".into()),
        work_order_item_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn inspection_model(name: &str, generates_work_order: bool) -> InspectionModel {
    InspectionModel {
        id: Some(5),
        uuid: Some("model-5".into()),
        tenant_id: Some(42),
        name: name.into(),
        generates_work_order,
        periodicity_days: None,
        observation: None,
        active: true,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn work_order(id: i64) -> WorkOrder {
    WorkOrder {
        id: Some(id),
        uuid: Some(format!("work-order-{id}")),
        tenant_id: Some(42),
        vehicle_id: 10,
        opened_at: None,
        odometer_km: 11_000.0,
        origin: WorkOrderOrigin::Manual,
        checklist_run_id: None,
        maintenance_plan_id: None,
        preventive_plan_id: None,
        service_type: None,
        description: "Existing repair".into(),
        responsible: None,
        status: WorkOrderStatus::Open,
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

fn work_order_item(id: i64, order_id: i64, description: &str) -> WorkOrderItem {
    WorkOrderItem {
        id: Some(id),
        uuid: Some(format!("work-order-item-{id}")),
        tenant_id: Some(42),
        work_order_id: order_id,
        description: description.into(),
        item_type: None,
        status: WorkOrderItemStatus::Pending,
        observation: None,
        resolved_by: None,
        resolved_at: None,
        resolution_description: None,
        purchase_order_id: None,
        is_purchase_placeholder: false,
        preventive_plan_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn preventive_plan() -> PreventivePlan {
    PreventivePlan {
        id: Some(8),
        uuid: Some("plan-8".into()),
        tenant_id: Some(42),
        vehicle_id: 10,
        plan_name: "Engine".into(),
        control_type: PreventiveControlType::Kilometers,
        interval_km: Some(10_000.0),
        interval_days: None,
        last_service_km: Some(0.0),
        last_service_date: None,
        extension_limit_km: None,
        last_work_order_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn preventive_alert(model: &str) -> PreventivePlanAlert {
    PreventivePlanAlert {
        id: Some(9),
        uuid: Some("alert-9".into()),
        tenant_id: Some(42),
        preventive_plan_id: 8,
        at_km: 5_000.0,
        title: "Intermediate check".into(),
        inspection_model: model.into(),
        discharged_cycle: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn use_case() -> (TechnicalInspectionUseCase, sea_orm::DatabaseConnection) {
    let db = MockDatabase::new(DatabaseBackend::MySql).into_connection();
    let store = TechnicalInspectionDatabaseGateway::new(
        TechnicalInspectionGateway::new(db.clone()),
        TechnicalInspectionItemGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        WorkOrderGateway::new(db.clone()),
        WorkOrderItemGateway::new(db.clone()),
        PreventivePlanGateway::new(db.clone()),
        PreventivePlanAlertGateway::new(db.clone()),
        InspectionModelGateway::new(db.clone()),
    );
    (TechnicalInspectionUseCase::new(store), db)
}

fn inspection(model: &str) -> TechnicalInspection {
    TechnicalInspection {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        vehicle_id: 10,
        inspection_model: model.into(),
        inspected_at: chrono::NaiveDate::from_ymd_opt(2026, 10, 2).expect("valid fixed date"),
        odometer_km: 100.0,
        observation: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn invalid_inspection_content_is_rejected_before_database_access() {
    let (use_case, db) = use_case();

    let missing_model = use_case
        .submit(inspection("  "), Vec::new())
        .await
        .expect_err("inspection model is required");
    let no_items = use_case
        .submit(inspection("Brake check"), Vec::new())
        .await
        .expect_err("at least one inspection answer is required");
    let blank_item = use_case
        .submit(
            inspection("Brake check"),
            vec![
                business::domain::technical_inspection_item::TechnicalInspectionItem {
                    id: None,
                    uuid: None,
                    tenant_id: Some(42),
                    technical_inspection_id: 0,
                    description: "  ".into(),
                    conforming: false,
                    observation: None,
                    work_order_item_id: None,
                    created_at: None,
                    created_by: None,
                    updated_at: None,
                    updated_by: None,
                },
            ],
        )
        .await
        .expect_err("item description is required");

    assert_eq!(missing_model.message, MODEL_REQUIRED);
    assert_eq!(no_items.message, AT_LEAST_ONE_ITEM_REQUIRED);
    assert_eq!(blank_item.message, ITEM_DESCRIPTION_REQUIRED);
    assert!(db.into_transaction_log().is_empty());
}

#[tokio::test]
async fn a_nonconforming_inspection_item_opens_one_work_order_and_uses_vehicle_odometer() {
    let (use_case, store) = memory_use_case(MemoryInspectionStore {
        vehicle: Some(memory_vehicle()),
        ..Default::default()
    });

    let mut submission = inspection("  Safety check  ");
    submission.odometer_km = 0.0;
    let (saved, items, opened) = use_case
        .submit(submission, vec![answer("Brake pads", false)])
        .await
        .expect("inspection and work order are recorded");

    assert_eq!(saved.inspection_model, "Safety check");
    assert_eq!(saved.odometer_km, 12_000.0);
    assert!(opened);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].work_order_item_id, Some(202));
    let stored = store.lock().unwrap();
    assert_eq!(stored.saved_orders.len(), 1);
    assert_eq!(stored.saved_orders[0].origin, WorkOrderOrigin::Inspection);
    assert_eq!(stored.saved_orders[0].odometer_km, 12_000.0);
}

#[tokio::test]
async fn a_known_model_that_does_not_generate_orders_records_answers_without_work_orders() {
    let (use_case, store) = memory_use_case(MemoryInspectionStore {
        vehicle: Some(memory_vehicle()),
        model: Some(inspection_model("Safety check", false)),
        ..Default::default()
    });
    let (_, items, opened) = use_case
        .submit(
            inspection("Safety check"),
            vec![answer("Broken reflector", false), answer("Horn", true)],
        )
        .await
        .expect("model may intentionally suppress work orders");

    assert!(!opened);
    assert!(items.iter().all(|item| item.work_order_item_id.is_none()));
    assert!(store.lock().unwrap().saved_orders.is_empty());
}

#[tokio::test]
async fn matching_defects_are_linked_once_to_pending_items_in_existing_work_orders() {
    let mut existing_item = work_order_item(77, 12, "Troca do Óleo");
    existing_item.observation = Some("Mechanic note".into());
    let (use_case, store) = memory_use_case(MemoryInspectionStore {
        vehicle: Some(memory_vehicle()),
        open_orders: vec![work_order(12)],
        work_order_items: vec![existing_item],
        ..Default::default()
    });

    let (saved, items, opened) = use_case
        .submit(
            inspection("Routine"),
            vec![
                answer("  troca oleo ", false),
                answer("Troca de óleo", false),
            ],
        )
        .await
        .expect("a matching existing defect is linked, and the second answer opens one new order");

    assert!(opened);
    assert_eq!(items[0].work_order_item_id, Some(77));
    assert_eq!(items[1].work_order_item_id, Some(202));
    let stored = store.lock().unwrap();
    assert_eq!(stored.saved_orders.len(), 1);
    assert!(stored.work_order_items.iter().any(|item| {
        item.id == Some(77)
            && item
                .observation
                .as_deref()
                .is_some_and(|note| note.contains("Inspection"))
    }));
    assert_eq!(saved.id, Some(101));
}

#[tokio::test]
async fn inspection_discharges_only_reached_alerts_for_its_model() {
    let (use_case, store) = memory_use_case(MemoryInspectionStore {
        vehicle: Some(memory_vehicle()),
        model: Some(inspection_model("Safety check", false)),
        plans: vec![preventive_plan()],
        alerts: vec![
            preventive_alert("Safety check"),
            preventive_alert("Another model"),
        ],
        ..Default::default()
    });

    let (_, _, opened) = use_case
        .submit(inspection("Safety check"), vec![answer("Brakes", true)])
        .await
        .expect("inspection can satisfy a reached preventive alert");

    assert!(!opened);
    let saved_alerts = store.lock().unwrap().saved_alerts.clone();
    assert_eq!(saved_alerts.len(), 1);
    assert_eq!(saved_alerts[0].inspection_model, "Safety check");
    assert_eq!(saved_alerts[0].discharged_cycle.as_deref(), Some("0|"));
}

#[tokio::test]
async fn inspection_requires_a_vehicle_owned_by_the_same_tenant() {
    let mut other_vehicle = memory_vehicle();
    other_vehicle.tenant_id = Some(99);
    let (use_case, _) = memory_use_case(MemoryInspectionStore {
        vehicle: Some(other_vehicle),
        ..Default::default()
    });
    let error = use_case
        .submit(inspection("Safety check"), vec![answer("Item", true)])
        .await
        .expect_err("cross-tenant vehicle is rejected");
    assert_eq!(error.message, NOT_A_TENANT_VEHICLE);
}

#[tokio::test]
async fn inspection_lookup_returns_items_and_reports_missing_inspections() {
    let mut stored = MemoryInspectionStore {
        inspection: Some(inspection("Safety check")),
        ..Default::default()
    };
    stored.inspection.as_mut().unwrap().id = Some(101);
    stored.inspection.as_mut().unwrap().uuid = Some("inspection-101".into());
    stored.inspection_items.push(answer("Brake pads", true));
    stored.inspection_items[0].id = Some(301);
    stored.inspection_items[0].technical_inspection_id = 101;
    let (use_case, _) = memory_use_case(stored);

    let (found, items) = use_case
        .find_by_uuid("inspection-101".into())
        .await
        .expect("inspection and its answers are returned");
    assert_eq!(found.id, Some(101));
    assert_eq!(items.len(), 1);
    assert_eq!(
        use_case
            .find_by_uuid("missing".into())
            .await
            .expect_err("unknown inspection UUID is reported")
            .message,
        INSPECTION_NOT_FOUND
    );
}

#[tokio::test]
async fn work_order_uuids_are_deduplicated_and_missing_relations_are_ignored() {
    let mut linked = work_order_item(77, 12, "Brake pads");
    let mut missing = work_order_item(78, 404, "Missing order");
    linked.id = Some(77);
    missing.id = Some(78);
    let mut second = linked.clone();
    second.id = Some(79);
    let answer_items = vec![
        TechnicalInspectionItem {
            work_order_item_id: Some(77),
            ..answer("Brake pads", false)
        },
        TechnicalInspectionItem {
            work_order_item_id: Some(79),
            ..answer("Brake pads", false)
        },
        TechnicalInspectionItem {
            work_order_item_id: Some(999),
            ..answer("Unknown item", false)
        },
        TechnicalInspectionItem {
            work_order_item_id: None,
            ..answer("No link", true)
        },
    ];
    let mut another_order = work_order(13);
    another_order.uuid = None;
    let store = MemoryInspectionStore {
        work_order_items: vec![linked, second, missing],
        open_orders: vec![work_order(12), another_order],
        ..MemoryInspectionStore::default()
    };
    let (use_case, _) = memory_use_case(store);
    let uuids = use_case
        .work_order_uuids(&answer_items)
        .await
        .expect("linked work orders are resolved");

    assert_eq!(uuids, vec!["work-order-12"]);
}
