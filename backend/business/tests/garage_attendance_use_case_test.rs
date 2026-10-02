use business::gateway::garage_attendance_gateway::GarageAttendanceGateway;
use business::gateway::garage_attendance_workflow_gateway::GarageAttendanceDatabaseGateway;
use business::gateway::garage_service_gateway::GarageServiceGateway;
use business::gateway::garage_service_log_gateway::GarageServiceLogGateway;
use business::gateway::garage_service_model_gateway::GarageServiceModelGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::use_cases::garage_attendance_use_case::{GarageAttendanceUseCase, VEHICLE_NOT_FOUND};
use sea_orm::{DatabaseBackend, MockDatabase};

#[tokio::test]
async fn opening_a_triage_for_a_missing_vehicle_stops_after_the_vehicle_lookup() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([Vec::<entity::vehicle_entity::Model>::new()])
        .into_connection();
    let use_case = GarageAttendanceUseCase::new(GarageAttendanceDatabaseGateway::new(
        GarageAttendanceGateway::new(db.clone()),
        GarageServiceGateway::new(db.clone()),
        GarageServiceLogGateway::new(db.clone()),
        GarageServiceModelGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
    ));

    let error = use_case
        .open(
            Some(42),
            10,
            None,
            business::domain::enums::GarageAttendanceOrigin::Manual,
        )
        .await
        .expect_err("a missing vehicle cannot enter the garage");

    assert_eq!(error.message, VEHICLE_NOT_FOUND);
    let queries = db.into_transaction_log();
    assert_eq!(queries.len(), 1);
    assert!(!format!("{queries:?}").contains("INSERT"));
}
