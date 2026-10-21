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
use sea_orm::{DatabaseBackend, DatabaseConnection, MockDatabase};

fn use_case(db: &DatabaseConnection) -> ChecklistRunUseCase {
    ChecklistRunUseCase::new(
        ChecklistRunDatabaseGateway::new(
            ChecklistRunGateway::new(db.clone()),
            ChecklistTemplateGateway::new(db.clone()),
            ChecklistTemplateItemGateway::new(db.clone()),
            ChecklistAnswerGateway::new(db.clone()),
            VehicleGateway::new(db.clone()),
            UserGateway::new(db.clone()),
            WorkOrderGateway::new(db.clone()),
            WorkOrderItemGateway::new(db.clone()),
        ),
        KmEvolutionUseCase::new(
            KmEvolutionGateway::new(db.clone()),
            VehicleGateway::new(db.clone()),
        ),
    )
}

fn run_row(id: i64, opening_checklist_id: Option<i64>) -> entity::checklist_run_entity::Model {
    let now = chrono::Utc::now();
    entity::checklist_run_entity::Model {
        id,
        uuid: business::commons::functions::string_to_bytes("684db325-63cb-470a-aa09-45811dd1904a"),
        tenant_id: Some(42),
        checklist_template_id: 7,
        driver_id: 5,
        vehicle_id: 10,
        checklist_type: if opening_checklist_id.is_some() {
            "Return"
        } else {
            "Departure"
        }
        .into(),
        odometer_km: 100.0,
        notes: None,
        opening_checklist_id,
        created_at: now,
        created_by: None,
        updated_at: now,
        updated_by: None,
    }
}

#[tokio::test]
async fn current_holder_distinguishes_absent_open_and_closed_departures() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([Vec::<entity::checklist_run_entity::Model>::new()])
        .into_connection();
    assert!(use_case(&db).current_holder(10).await.unwrap().is_none());
    assert_eq!(db.into_transaction_log().len(), 1);

    for closed in [false, true] {
        let returns = if closed {
            vec![run_row(12, Some(11))]
        } else {
            Vec::new()
        };
        let db = MockDatabase::new(DatabaseBackend::MySql)
            .append_query_results([[run_row(11, None)]])
            .append_query_results([returns])
            .into_connection();
        let holder = use_case(&db).current_holder(10).await.unwrap();
        if closed {
            assert!(holder.is_none(), "a returned vehicle has no current holder");
        } else {
            let holder = holder.expect("unclosed departure identifies the holder");
            assert_eq!(holder.driver_id, 5);
            assert_eq!(holder.id, Some(11));
        }
        assert_eq!(db.into_transaction_log().len(), 2);
    }
}

#[tokio::test]
async fn checklist_lookup_returns_answers_and_reports_missing_checklists() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([[run_row(11, None)]])
        .append_query_results([Vec::<entity::checklist_answer_entity::Model>::new()])
        .append_query_results([Vec::<entity::checklist_run_entity::Model>::new()])
        .into_connection();
    let use_case = use_case(&db);
    let (run, answers) = use_case
        .find_by_uuid("684db325-63cb-470a-aa09-45811dd1904a".into())
        .await
        .expect("persisted checklist can be read");
    assert_eq!(run.id, Some(11));
    assert!(answers.is_empty());
    assert_eq!(
        use_case
            .find_by_uuid("missing".into())
            .await
            .unwrap_err()
            .message,
        "Checklist not found"
    );
}

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
