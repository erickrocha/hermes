//! EPIC-XF-07-S01 (HRMS-033, D-8): AD-002 claims `business` is testable with
//! no database -- the `mock` cargo feature and this `[[test]]` target exist
//! specifically to evidence that claim, and until now this file was 0 bytes.
//! These tests exercise the real gateway code (not just the pure query
//! builder already covered by `commons::gateway`'s unit tests) against a
//! `sea_orm::MockDatabase`, so what's proven here is that tenant scoping
//! actually reaches the database call a gateway makes, not only that the
//! query-building function is capable of producing the right SQL in
//! isolation.

use business::commons::gateway::Gateway;
use business::gateway::user_gateway::UserGateway;
use entity::audit::{AuditUser, run_with_user};
use sea_orm::{DatabaseBackend, MockDatabase, Transaction};

fn user(tenant_id: Option<i64>, enforce_tenant: bool) -> AuditUser {
    AuditUser {
        id: 1,
        email: "actor@example.com".to_string(),
        tenant_id,
        enforce_tenant,
    }
}

#[tokio::test]
async fn tenant_scoped_find_by_id_sends_the_tenant_filter_to_the_database() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([Vec::<entity::user_entity::Model>::new()])
        .into_connection();

    let gateway = UserGateway::new(db.clone());
    run_with_user(Some(user(Some(42), true)), gateway.find_by_id(1))
        .await
        .expect("mocked query succeeds with an empty result set");

    let log: &[Transaction] = &db.into_transaction_log();
    assert_eq!(log.len(), 1, "exactly one query should have been sent");
    let sql = format!("{:?}", log[0]);
    assert!(
        sql.contains("tenant_id"),
        "query did not filter by tenant_id: {sql}"
    );
    assert!(
        sql.contains("42"),
        "query did not filter by the actor's tenant id: {sql}"
    );
}

#[tokio::test]
async fn unrestricted_scope_sends_no_tenant_filter_to_the_database() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([Vec::<entity::user_entity::Model>::new()])
        .into_connection();

    let gateway = UserGateway::new(db.clone());
    // An unbound platform administrator: enforce_tenant is false.
    run_with_user(Some(user(None, false)), gateway.find_by_id(1))
        .await
        .expect("mocked query succeeds with an empty result set");

    let log: &[Transaction] = &db.into_transaction_log();
    assert_eq!(log.len(), 1);
    assert!(
        !format!("{:?}", log[0])
            .to_lowercase()
            .contains("tenant_id ="),
        "an unbound administrator's query should carry no tenant_id filter: {:?}",
        log[0]
    );
}

#[tokio::test]
async fn denied_scope_sends_a_query_that_matches_no_rows() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([Vec::<entity::user_entity::Model>::new()])
        .into_connection();

    let gateway = UserGateway::new(db.clone());
    // A non-admin token with no tenant at all -- unreachable in practice
    // (the auth middleware 403s this shape before any gateway runs), but
    // exactly the case tenant_delete/tenant_select must still refuse.
    run_with_user(Some(user(None, true)), gateway.find_by_id(1))
        .await
        .expect("mocked query succeeds with an empty result set");

    let log: &[Transaction] = &db.into_transaction_log();
    assert_eq!(log.len(), 1);
    assert!(
        format!("{:?}", log[0]).contains("1 = 0"),
        "a denied caller's query should be forced to match no rows: {:?}",
        log[0]
    );
}
