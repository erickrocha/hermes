use business::gateway::access_profile_gateway::{AccessProfileGateway, AccessProfileStore};
use business::gateway::billing_gateway::{BillingGateway, BillingStore};
use chrono::NaiveDate;
use entity::audit::{AuditUser, run_with_user};
use entity::{access_profile_entity, billing_charge_entity, tenant_billing_entity, user_entity};
use sea_orm::{DatabaseBackend, MockDatabase, MockExecResult, Value};
use std::collections::BTreeMap;

fn count(value: i64) -> Vec<Vec<BTreeMap<&'static str, Value>>> {
    vec![vec![BTreeMap::from([(
        "num_items",
        Value::BigInt(Some(value)),
    )])]]
}

#[tokio::test]
async fn access_profile_and_billing_gateways_read_their_tenant_scoped_data() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([Vec::<access_profile_entity::Model>::new()])
        .append_query_results([Vec::<access_profile_entity::Model>::new()])
        .append_query_results([Vec::<access_profile_entity::Model>::new()])
        .append_query_results(count(4))
        .append_query_results([Vec::<access_profile_entity::Model>::new()])
        .append_query_results([Vec::<user_entity::Model>::new()])
        .append_exec_results([MockExecResult {
            last_insert_id: 0,
            rows_affected: 1,
        }])
        .append_query_results(count(3))
        .append_query_results([Vec::<tenant_billing_entity::Model>::new()])
        .append_query_results(count(5))
        .append_query_results([Vec::<billing_charge_entity::Model>::new()])
        .append_query_results([Vec::<billing_charge_entity::Model>::new()])
        .append_query_results(count(2))
        .append_query_results([Vec::<billing_charge_entity::Model>::new()])
        .into_connection();
    let profiles = AccessProfileGateway::new(db.clone());
    let billing = BillingGateway::new(db);
    let user = AuditUser {
        id: 1,
        email: "owner@example.com".into(),
        tenant_id: Some(42),
        enforce_tenant: true,
    };
    let date = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
    let period_start = date.and_hms_opt(0, 0, 0).unwrap();

    let result = run_with_user(Some(user), async {
        Ok::<_, business::domain::business_error::BusinessError>((
            profiles.get_by_id(4).await?,
            profiles.get_by_uuid("missing".into()).await?,
            profiles.find_by_key("operator", Some(42)).await?,
            profiles.list_page(0, 10).await?,
            profiles.user_profile_id(9).await?,
            profiles.set_user_profile(9, Some(4)).await?,
            profiles.users_holding(4).await?,
            billing.get_settings(42).await?,
            billing.count_active_human_users(42).await?,
            billing.find_charge_by_uuid("missing").await?,
            billing.find_charge_for_period(42, period_start).await?,
            billing.list_charges(0, 10).await?,
        ))
    })
    .await
    .unwrap();

    assert!(result.0.is_none());
    assert!(result.1.is_none());
    assert!(result.2.is_none());
    assert_eq!(result.3.1, 4);
    assert!(result.3.0.is_empty());
    assert!(result.4.is_none());
    assert_eq!(result.6, 3);
    assert!(result.7.is_none());
    assert_eq!(result.8, 5);
    assert!(result.9.is_none());
    assert!(result.10.is_none());
    assert_eq!(result.11.1, 2);
    assert!(result.11.0.is_empty());
}
