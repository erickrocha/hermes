use business::domain::technical_inspection::TechnicalInspection;
use business::gateway::inspection_model_gateway::InspectionModelGateway;
use business::gateway::preventive_plan_alert_gateway::PreventivePlanAlertGateway;
use business::gateway::preventive_plan_gateway::PreventivePlanGateway;
use business::gateway::technical_inspection_gateway::TechnicalInspectionGateway;
use business::gateway::technical_inspection_item_gateway::TechnicalInspectionItemGateway;
use business::gateway::technical_inspection_workflow_gateway::TechnicalInspectionDatabaseGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::gateway::work_order_gateway::WorkOrderGateway;
use business::gateway::work_order_item_gateway::WorkOrderItemGateway;
use business::use_cases::technical_inspection_use_case::{
    AT_LEAST_ONE_ITEM_REQUIRED, ITEM_DESCRIPTION_REQUIRED, MODEL_REQUIRED,
    TechnicalInspectionUseCase,
};
use sea_orm::{DatabaseBackend, MockDatabase};

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
