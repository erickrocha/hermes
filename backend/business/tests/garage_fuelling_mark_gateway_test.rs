use business::gateway::fuel_entry_gateway::FuelEntryGateway;
use business::gateway::garage_attendance_gateway::GarageAttendanceGateway;
use business::gateway::garage_fuelling_mark_gateway::{
    GarageFuellingMarkDatabaseGateway, GarageFuellingMarkStore,
};
use business::gateway::garage_service_gateway::GarageServiceGateway;
use business::gateway::garage_service_log_gateway::GarageServiceLogGateway;
use business::gateway::garage_service_model_gateway::GarageServiceModelGateway;
use business::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway;
use business::gateway::vehicle_presence_event_gateway::VehiclePresenceEventGateway;
use entity::audit::{AuditUser, run_with_user};
use entity::{
    fuel_entry_entity, garage_attendance_entity, garage_service_entity,
    garage_service_model_entity, tenant_rule_setting_entity, vehicle_presence_event_entity,
};
use sea_orm::{DatabaseBackend, MockDatabase};

#[tokio::test]
async fn fuelling_gateway_reads_each_marking_input_and_uses_default_rules() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([Vec::<garage_attendance_entity::Model>::new()])
        .append_query_results([Vec::<garage_attendance_entity::Model>::new()])
        .append_query_results([Vec::<fuel_entry_entity::Model>::new()])
        .append_query_results([Vec::<garage_service_entity::Model>::new()])
        .append_query_results([Vec::<garage_service_model_entity::Model>::new()])
        .append_query_results([Vec::<tenant_rule_setting_entity::Model>::new()])
        .append_query_results([Vec::<vehicle_presence_event_entity::Model>::new()])
        .into_connection();
    let gateway = GarageFuellingMarkDatabaseGateway::new(
        GarageAttendanceGateway::new(db.clone()),
        GarageServiceGateway::new(db.clone()),
        GarageServiceLogGateway::new(db.clone()),
        GarageServiceModelGateway::new(db.clone()),
        FuelEntryGateway::new(db.clone()),
        VehiclePresenceEventGateway::new(db.clone()),
        TenantRuleSettingGateway::new(db),
    );
    let user = AuditUser {
        id: 1,
        email: "owner@example.com".into(),
        tenant_id: Some(42),
        enforce_tenant: true,
    };

    let result = run_with_user(Some(user), async {
        Ok::<_, business::domain::business_error::BusinessError>((
            gateway.active_attendances().await?,
            gateway.active_attendance(7).await?,
            gateway.latest_full_tank_fuel(7).await?,
            gateway.services(9).await?,
            gateway.service_model(3).await?,
            gateway.rules(Some(42)).await?,
            gateway.last_arrival(7).await?,
        ))
    })
    .await
    .unwrap();

    assert!(result.0.is_empty());
    assert!(result.1.is_none());
    assert!(result.2.is_none());
    assert!(result.3.is_empty());
    assert!(result.4.is_none());
    assert_eq!(result.5, Default::default());
    assert!(result.6.is_none());
}
