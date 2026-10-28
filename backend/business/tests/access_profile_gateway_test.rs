//! `C-035`: the access-profile gateway's own reads -- what a tenant scope sees (its own profiles and the
//! system ones), what the platform sees, what a denied scope sees, and the rows coming back as domain.

use business::gateway::access_profile_gateway::{AccessProfileGateway, AccessProfileStore};
use chrono::{TimeZone, Utc};
use entity::access_profile_entity;
use entity::audit::{AuditUser, run_as_platform, run_with_user};
use sea_orm::{DatabaseBackend, MockDatabase, MockExecResult, Value};
use std::collections::BTreeMap;

fn count(value: i64) -> Vec<Vec<BTreeMap<&'static str, Value>>> {
    vec![vec![BTreeMap::from([("num_items", Value::BigInt(Some(value)))])]]
}

fn row(id: i64, tenant_id: Option<i64>, key: Option<&str>, permissions: &str) -> access_profile_entity::Model {
    access_profile_entity::Model {
        id,
        uuid: vec![id as u8; 16],
        tenant_id,
        name: key.unwrap_or("Dispatch").to_string(),
        name_key: key.unwrap_or("dispatch").to_lowercase(),
        scope: "Tenant".into(),
        system: key.is_some(),
        key: key.map(str::to_string),
        active: true,
        permissions: permissions.into(),
        created_at: Utc.with_ymd_and_hms(2026, 10, 28, 12, 0, 0).unwrap(),
        created_by: None,
        updated_at: Utc.with_ymd_and_hms(2026, 10, 28, 12, 0, 0).unwrap(),
        updated_by: None,
    }
}

#[tokio::test]
async fn a_tenant_reads_its_own_and_the_system_profiles_as_domain() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([vec![row(1, Some(42), None, r#"["vehicle.read@own"]"#)]])
        .append_query_results([vec![row(2, None, Some("Driver"), r#"["driver.act"]"#)]])
        .append_query_results([vec![row(1, Some(42), None, "not json")]])
        .append_query_results(count(2))
        .append_query_results([vec![row(2, None, Some("Driver"), "[]"), row(1, Some(42), None, "[]")]])
        .append_exec_results([MockExecResult { last_insert_id: 0, rows_affected: 1 }])
        .into_connection();
    let profiles = AccessProfileGateway::new(db);
    let user = AuditUser { id: 1, email: "owner@example.com".into(), tenant_id: Some(42), enforce_tenant: true };

    let (own, system, malformed, (page, total), extras) = run_with_user(Some(user), async {
        Ok::<_, business::domain::business_error::BusinessError>((
            profiles.get_by_uuid("01".repeat(16)).await?,
            profiles.find_by_key("Driver").await?,
            profiles.find_by_name_key("dispatch", None).await?,
            profiles.list_page(0, 10).await?,
            profiles.set_user_access(9, 1, &["vehicle.read".to_string()]).await?,
        ))
    })
    .await
    .unwrap();

    let own = own.expect("the tenant's own profile");
    assert_eq!((own.tenant_id, own.system, own.permissions), (Some(42), false, vec!["vehicle.read@own".to_string()]));
    let system = system.expect("the system profile");
    assert_eq!((system.tenant_id, system.system, system.key.as_deref()), (None, true, Some("Driver")));
    assert!(malformed.expect("row").permissions.is_empty(), "unreadable grants read as none, never as a panic");
    assert_eq!((page.len(), total), (2, 2));
    assert_eq!(extras, ());
}

#[tokio::test]
async fn the_platform_reads_everything_and_a_denied_scope_reads_nothing() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results(count(2))
        .append_query_results([vec![row(1, Some(42), None, "[]"), row(3, Some(7), None, "[]")]])
        .append_query_results([vec![row(1, Some(42), None, "[]")]])
        .append_query_results([Vec::<access_profile_entity::Model>::new()])
        .into_connection();
    let profiles = AccessProfileGateway::new(db);

    let (page, by_id) = run_as_platform(async {
        Ok::<_, business::domain::business_error::BusinessError>((profiles.list_page(0, 10).await?, profiles.get_by_id(1).await?))
    })
    .await
    .unwrap();
    assert_eq!(page.1, 2);
    assert!(by_id.is_some());

    let denied = AuditUser { id: 1, email: "nobody@example.com".into(), tenant_id: None, enforce_tenant: true };
    let none = run_with_user(Some(denied), async { profiles.get_by_id(1).await }).await.unwrap();
    assert!(none.is_none());
}
