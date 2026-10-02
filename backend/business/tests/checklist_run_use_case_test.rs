use business::domain::checklist_run::ChecklistRun;
use business::domain::enums::ChecklistType;
use business::gateway::checklist_answer_gateway::ChecklistAnswerGateway;
use business::gateway::checklist_run_gateway::{ChecklistRunDatabaseGateway, ChecklistRunGateway};
use business::gateway::checklist_template_gateway::ChecklistTemplateGateway;
use business::gateway::checklist_template_item_gateway::ChecklistTemplateItemGateway;
use business::gateway::km_evolution_gateway::KmEvolutionGateway;
use business::gateway::user_gateway::UserGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::gateway::work_order_gateway::WorkOrderGateway;
use business::gateway::work_order_item_gateway::WorkOrderItemGateway;
use business::use_cases::checklist_run_use_case::{ChecklistRunUseCase, NOT_A_TENANT_TEMPLATE};
use business::use_cases::km_evolution_use_case::KmEvolutionUseCase;
use sea_orm::{DatabaseBackend, MockDatabase};

#[tokio::test]
async fn a_template_outside_the_tenant_stops_checklist_creation_before_other_reads() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([Vec::<entity::checklist_template_entity::Model>::new()])
        .into_connection();
    let store = ChecklistRunDatabaseGateway::new(
        ChecklistRunGateway::new(db.clone()),
        ChecklistTemplateGateway::new(db.clone()),
        ChecklistTemplateItemGateway::new(db.clone()),
        ChecklistAnswerGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        UserGateway::new(db.clone()),
        WorkOrderGateway::new(db.clone()),
        WorkOrderItemGateway::new(db.clone()),
    );
    let km = KmEvolutionUseCase::new(
        KmEvolutionGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
    );
    let use_case = ChecklistRunUseCase::new(store, km);
    let run = ChecklistRun {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        checklist_template_id: 7,
        driver_id: 5,
        vehicle_id: 10,
        checklist_type: ChecklistType::Departure,
        odometer_km: 100.0,
        notes: None,
        opening_checklist_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };

    let error = use_case
        .create(run, Vec::new())
        .await
        .expect_err("template does not belong to this tenant");

    assert_eq!(error.message, NOT_A_TENANT_TEMPLATE);
    assert_eq!(db.into_transaction_log().len(), 1);
}
