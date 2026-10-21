use business::domain::enums::{MaintenancePlanStatus, WorkOrderOrigin, WorkOrderStatus};
use business::domain::work_order::WorkOrder;
use business::gateway::non_execution_gateway::{NonExecutionGateway, NonExecutionStore};
use chrono::NaiveDate;
use entity::audit::{AuditUser, run_with_user};
use entity::{
    maintenance_plan_entity, non_execution_occurrence_entity, non_execution_record_entity,
    vehicle_presence_event_entity, work_order_entity,
};
use sea_orm::{DatabaseBackend, MockDatabase, Value};
use std::collections::BTreeMap;

fn count(value: i64) -> Vec<Vec<BTreeMap<&'static str, Value>>> {
    vec![vec![BTreeMap::from([(
        "num_items",
        Value::BigInt(Some(value)),
    )])]]
}

fn work_order_without_id() -> WorkOrder {
    WorkOrder {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        vehicle_id: 7,
        opened_at: None,
        odometer_km: 10_000.0,
        origin: WorkOrderOrigin::Manual,
        checklist_run_id: None,
        maintenance_plan_id: None,
        preventive_plan_id: None,
        service_type: None,
        description: "Check vehicle".into(),
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
async fn gateway_handles_empty_tenant_scoped_queries_without_inventing_records() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([Vec::<maintenance_plan_entity::Model>::new()])
        .append_query_results([Vec::<maintenance_plan_entity::Model>::new()])
        .append_query_results([Vec::<work_order_entity::Model>::new()])
        .append_query_results([Vec::<work_order_entity::Model>::new()])
        .append_query_results([Vec::<vehicle_presence_event_entity::Model>::new()])
        .append_query_results([Vec::<vehicle_presence_event_entity::Model>::new()])
        .append_query_results([Vec::<non_execution_record_entity::Model>::new()])
        .append_query_results(count(0))
        .append_query_results([Vec::<non_execution_occurrence_entity::Model>::new()])
        .append_query_results([Vec::<non_execution_occurrence_entity::Model>::new()])
        .into_connection();
    let gateway = NonExecutionGateway::new(db);
    let user = AuditUser {
        id: 1,
        email: "owner@example.com".into(),
        tenant_id: Some(42),
        enforce_tenant: true,
    };
    let date = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
    let from = date.and_hms_opt(8, 0, 0).unwrap();
    let to = date.and_hms_opt(18, 0, 0).unwrap();

    let result = run_with_user(Some(user), async {
        Ok::<_, business::domain::business_error::BusinessError>((
            gateway
                .plans_in_status(&[MaintenancePlanStatus::Scheduled])
                .await?,
            gateway.find_plan(2).await?,
            gateway.work_orders_of_plan(3).await?,
            gateway.find_work_order_by_uuid("missing".into()).await?,
            gateway.presence_events(7, from, to).await?,
            gateway.last_presence_before(7, from).await?,
            gateway.find_record_by_work_order(8).await?,
            gateway.occurrence_key_exists("missing").await?,
            gateway.find_occurrence_by_uuid("missing".into()).await?,
            gateway
                .history_of_work_order(work_order_without_id())
                .await?,
            gateway.pending().await?,
        ))
    })
    .await
    .unwrap();

    assert!(result.0.is_empty());
    assert!(result.1.is_none());
    assert!(result.2.is_empty());
    assert!(result.3.is_none());
    assert!(result.4.is_empty());
    assert!(result.5.is_none());
    assert!(result.6.is_none());
    assert!(!result.7);
    assert!(result.8.is_none());
    assert!(result.9.is_none());
    assert!(result.10.is_empty());
}
