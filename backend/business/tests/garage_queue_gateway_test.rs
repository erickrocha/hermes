use business::gateway::garage_attendance_gateway::GarageAttendanceGateway;
use business::gateway::garage_cycle_gateway::GarageCycleGateway;
use business::gateway::garage_queue_gateway::{GarageQueueDatabaseGateway, GarageQueueStore};
use business::gateway::garage_service_gateway::GarageServiceGateway;
use business::gateway::garage_service_model_gateway::GarageServiceModelGateway;
use business::gateway::preventive_plan_gateway::PreventivePlanGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use entity::audit::{AuditUser, run_with_user};
use entity::{
    garage_attendance_entity, garage_cycle_state_entity, garage_service_entity,
    garage_service_model_entity, preventive_plan_entity, vehicle_entity,
};
use sea_orm::{DatabaseBackend, MockDatabase, Value};
use std::collections::BTreeMap;

#[tokio::test]
async fn queue_gateway_reads_each_input_from_the_callers_tenant() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([Vec::<garage_attendance_entity::Model>::new()])
        .append_query_results([Vec::<vehicle_entity::Model>::new()])
        .append_query_results([Vec::<vehicle_entity::Model>::new()])
        .append_query_results([Vec::<garage_service_entity::Model>::new()])
        .append_query_results([Vec::<garage_service_model_entity::Model>::new()])
        .append_query_results([Vec::<garage_service_model_entity::Model>::new()])
        .append_query_results([Vec::<BTreeMap<&'static str, Value>>::new()])
        .append_query_results([Vec::<garage_service_entity::Model>::new()])
        .append_query_results([Vec::<garage_attendance_entity::Model>::new()])
        .append_query_results([Vec::<preventive_plan_entity::Model>::new()])
        .append_query_results([Vec::<garage_cycle_state_entity::Model>::new()])
        .into_connection();
    let store = GarageQueueDatabaseGateway::new(
        GarageAttendanceGateway::new(db.clone()),
        GarageServiceGateway::new(db.clone()),
        GarageServiceModelGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        PreventivePlanGateway::new(db.clone()),
        GarageCycleGateway::new(db),
    );
    let user = AuditUser {
        id: 1,
        email: "owner@example.com".into(),
        tenant_id: Some(42),
        enforce_tenant: true,
    };

    let result = run_with_user(Some(user), async {
        Ok::<_, business::domain::business_error::BusinessError>((
            store.active_attendances().await?,
            store.all_vehicles().await?,
            store.find_vehicle(7).await?,
            store.services(11).await?,
            store.all_service_models().await?,
            store.active_service_models().await?,
            store.attendance_ids_by_vehicle(7).await?,
            store.performed_services_by_attendances(vec![11]).await?,
            store.latest_attendance(7).await?,
            store.preventive_plans_for_vehicle(7).await?,
            store.garage_cycle_state(7).await?,
        ))
    })
    .await
    .unwrap();

    assert!(result.0.is_empty());
    assert!(result.1.is_empty());
    assert!(result.2.is_none());
    assert!(result.3.is_empty());
    assert!(result.4.is_empty());
    assert!(result.5.is_empty());
    assert!(result.6.is_empty());
    assert!(result.7.is_empty());
    assert!(result.8.is_none());
    assert!(result.9.is_empty());
    assert!(result.10.is_none());
}
