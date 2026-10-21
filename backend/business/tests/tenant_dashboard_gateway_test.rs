use business::gateway::tenant_dashboard_gateway::{TenantDashboardGateway, TenantDashboardStore};
use entity::audit::{AuditUser, run_with_user};
use entity::billing_charge_entity;
use sea_orm::{DatabaseBackend, MockDatabase, Value};
use std::collections::BTreeMap;

fn count(value: i64) -> Vec<Vec<BTreeMap<&'static str, Value>>> {
    vec![vec![BTreeMap::from([(
        "num_items",
        Value::BigInt(Some(value)),
    )])]]
}

#[tokio::test]
async fn dashboard_loads_each_tenant_total_and_pending_charge_window() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results(count(4))
        .append_query_results(count(1))
        .append_query_results(count(3))
        .append_query_results(count(2))
        .append_query_results(count(5))
        .append_query_results(count(6))
        .append_query_results(count(7))
        .append_query_results(count(8))
        .append_query_results([Vec::<billing_charge_entity::Model>::new()])
        .into_connection();
    let gateway = TenantDashboardGateway::new(db);
    let user = AuditUser {
        id: 1,
        email: "owner@example.com".into(),
        tenant_id: Some(42),
        enforce_tenant: true,
    };

    let totals = run_with_user(Some(user), async {
        Ok::<_, business::domain::business_error::BusinessError>((
            gateway.active_vehicles().await?,
            gateway.vehicles_in_maintenance().await?,
            gateway.open_work_orders().await?,
            gateway.work_orders_awaiting_parts().await?,
            gateway.yard_queue().await?,
            gateway.unread_maintenance_notices().await?,
            gateway.unjustified_non_executions().await?,
            gateway.enabled_users().await?,
            gateway.pending_charges().await?,
        ))
    })
    .await
    .unwrap();

    assert_eq!(
        (
            totals.0, totals.1, totals.2, totals.3, totals.4, totals.5, totals.6, totals.7,
        ),
        (4, 1, 3, 2, 5, 6, 7, 8)
    );
    assert!(totals.8.is_empty());
}
