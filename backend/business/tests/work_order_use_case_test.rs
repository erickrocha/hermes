use business::domain::enums::{WorkOrderOrigin, WorkOrderStatus};
use business::domain::work_order::WorkOrder;
use business::gateway::km_evolution_gateway::KmEvolutionGateway;
use business::gateway::preventive_plan_extension_gateway::PreventivePlanExtensionGateway;
use business::gateway::preventive_plan_gateway::PreventivePlanGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::gateway::work_order_gateway::{WorkOrderGateway, WorkOrderWorkflowDatabaseGateway};
use business::gateway::work_order_item_gateway::WorkOrderItemGateway;
use business::use_cases::km_evolution_use_case::KmEvolutionUseCase;
use business::use_cases::work_order_use_case::{
    DESCRIPTION_REQUIRED, NEGATIVE_ODOMETER, WorkOrderUseCase,
};
use sea_orm::{DatabaseBackend, MockDatabase};

fn use_case() -> (WorkOrderUseCase, sea_orm::DatabaseConnection) {
    let db = MockDatabase::new(DatabaseBackend::MySql).into_connection();
    let store = WorkOrderWorkflowDatabaseGateway::new(
        WorkOrderGateway::new(db.clone()),
        WorkOrderItemGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        PreventivePlanGateway::new(db.clone()),
        PreventivePlanExtensionGateway::new(db.clone()),
    );
    let km = KmEvolutionUseCase::new(
        KmEvolutionGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
    );
    (WorkOrderUseCase::new(store, km), db)
}

fn work_order(description: &str, odometer_km: f64) -> WorkOrder {
    WorkOrder {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        vehicle_id: 10,
        opened_at: None,
        odometer_km,
        origin: WorkOrderOrigin::Manual,
        checklist_run_id: None,
        maintenance_plan_id: None,
        preventive_plan_id: None,
        service_type: None,
        description: description.into(),
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

#[tokio::test]
async fn invalid_work_order_input_is_rejected_before_any_store_or_odometer_access() {
    let (use_case, db) = use_case();

    let blank = use_case
        .create(work_order("  ", 100.0))
        .await
        .expect_err("description is required");
    let negative = use_case
        .create(work_order("Inspect brakes", -1.0))
        .await
        .expect_err("odometer cannot be negative");

    assert_eq!(blank.message, DESCRIPTION_REQUIRED);
    assert_eq!(negative.message, NEGATIVE_ODOMETER);
    assert!(db.into_transaction_log().is_empty());
}
