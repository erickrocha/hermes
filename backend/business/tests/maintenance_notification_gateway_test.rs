use business::gateway::maintenance_notification_gateway::{
    MaintenanceNotificationGateway, MaintenanceNotificationStore,
};
use chrono::NaiveDate;
use entity::audit::{AuditUser, run_with_user};
use entity::maintenance_notification_entity;
use sea_orm::{DatabaseBackend, MockDatabase, Value};
use std::collections::BTreeMap;

fn count(value: i64) -> Vec<Vec<BTreeMap<&'static str, Value>>> {
    vec![vec![BTreeMap::from([(
        "num_items",
        Value::BigInt(Some(value)),
    )])]]
}

#[tokio::test]
async fn notification_gateway_reads_pages_and_leaves_empty_unread_sets_unchanged() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results(count(0))
        .append_query_results([Vec::<maintenance_notification_entity::Model>::new()])
        .append_query_results(count(1))
        .append_query_results([Vec::<maintenance_notification_entity::Model>::new()])
        .append_query_results(count(2))
        .append_query_results([Vec::<BTreeMap<&'static str, Value>>::new()])
        .append_query_results([Vec::<maintenance_notification_entity::Model>::new()])
        .into_connection();
    let gateway = MaintenanceNotificationGateway::new(db);
    let user = AuditUser {
        id: 1,
        email: "owner@example.com".into(),
        tenant_id: Some(42),
        enforce_tenant: true,
    };
    let now = NaiveDate::from_ymd_opt(2026, 10, 21)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap();

    let result = run_with_user(Some(user), async {
        Ok::<_, business::domain::business_error::BusinessError>((
            gateway.find_page(false, 0, 10).await?,
            gateway.find_page(true, 0, 10).await?,
            gateway.count_unread().await?,
            gateway.mark_all_read(now).await?,
            gateway.find_by_uuid("missing".into()).await?,
        ))
    })
    .await
    .unwrap();

    assert_eq!(result.0.1, 0);
    assert!(result.0.0.is_empty());
    assert_eq!(result.1.1, 1);
    assert!(result.1.0.is_empty());
    assert_eq!(result.2, 2);
    assert_eq!(result.3, 0);
    assert!(result.4.is_none());
}
