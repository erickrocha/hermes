use business::gateway::garage_release_gateway::{GarageReleaseDatabaseGateway, GarageReleaseStore};
use entity::audit::{AuditUser, run_with_user};
use entity::{
    garage_attendance_entity, garage_service_entity, garage_service_model_entity,
    km_evolution_entity, work_order_entity, work_order_item_entity,
};
use sea_orm::{DatabaseBackend, MockDatabase};

#[tokio::test]
async fn release_gateway_reads_all_inputs_as_empty_for_a_tenant() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([Vec::<garage_attendance_entity::Model>::new()])
        .append_query_results([Vec::<garage_service_entity::Model>::new()])
        .append_query_results([Vec::<garage_service_model_entity::Model>::new()])
        .append_query_results([Vec::<km_evolution_entity::Model>::new()])
        .append_query_results([Vec::<work_order_entity::Model>::new()])
        .append_query_results([Vec::<work_order_item_entity::Model>::new()])
        .into_connection();
    let gateway = GarageReleaseDatabaseGateway::new(db);
    let user = AuditUser {
        id: 1,
        email: "owner@example.com".into(),
        tenant_id: Some(42),
        enforce_tenant: true,
    };

    let result = run_with_user(Some(user), async {
        Ok::<_, business::domain::business_error::BusinessError>((
            gateway.find_attendance("missing".into()).await?,
            gateway.list_services(7).await?,
            gateway.find_service_display_name(8).await?,
            gateway.latest_odometer(7).await?,
            gateway.find_active_work_order(7).await?,
            gateway.list_work_order_items(9).await?,
        ))
    })
    .await
    .unwrap();

    assert!(result.0.is_none());
    assert!(result.1.is_empty());
    assert!(result.2.is_none());
    assert!(result.3.is_none());
    assert!(result.4.is_none());
    assert!(result.5.is_empty());
}
