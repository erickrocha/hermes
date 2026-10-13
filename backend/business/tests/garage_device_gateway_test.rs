use business::gateway::garage_device_gateway::{GarageDeviceGateway, GarageDeviceStore};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use entity::garage_device_entity;
use sea_orm::{DatabaseBackend, MockDatabase, MockExecResult};

fn device() -> garage_device_entity::Model {
    let now = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2026, 10, 13).unwrap(),
        NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
    )
    .and_utc();
    garage_device_entity::Model {
        id: 7,
        uuid: uuid::Uuid::new_v4().as_bytes().to_vec(),
        tenant_id: Some(42),
        name: "Garage wall".into(),
        credential_version: 1,
        pairing_code: Some("one-time-code".into()),
        pairing_code_expires_at: Some(now + chrono::Duration::minutes(10)),
        paired_at: None,
        revoked_at: None,
        created_at: now,
        created_by: None,
        updated_at: now,
        updated_by: None,
    }
}

#[tokio::test]
async fn pairing_code_consumption_is_a_conditional_single_row_update() {
    let now = NaiveDate::from_ymd_opt(2026, 10, 13)
        .unwrap()
        .and_hms_opt(12, 1, 0)
        .unwrap();
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([vec![device()]])
        .append_exec_results([MockExecResult {
            last_insert_id: 0,
            rows_affected: 1,
        }])
        .into_connection();
    let gateway = GarageDeviceGateway::new(db.clone());

    let consumed = entity::audit::run_for_tenant(
        42,
        "garage-device-pairing",
        gateway.consume_pairing_code("one-time-code".into(), now),
    )
    .await
    .expect("consume query succeeds")
    .expect("one row wins the compare-and-set");

    assert_eq!(consumed.paired_at, Some(now));
    assert_eq!(consumed.pairing_code, None);
    let sql = format!("{:?}", db.into_transaction_log());
    assert!(sql.contains("pairing_code_expires_at"), "{sql}");
    assert!(sql.contains("paired_at"), "{sql}");
    assert!(sql.contains("revoked_at"), "{sql}");
    assert!(sql.contains("tenant_id"), "{sql}");
}

#[tokio::test]
async fn a_lost_pairing_race_reports_no_consumed_device() {
    let now = NaiveDate::from_ymd_opt(2026, 10, 13)
        .unwrap()
        .and_hms_opt(12, 1, 0)
        .unwrap();
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([vec![device()]])
        .append_exec_results([MockExecResult {
            last_insert_id: 0,
            rows_affected: 0,
        }])
        .into_connection();
    let gateway = GarageDeviceGateway::new(db);

    let consumed = entity::audit::run_for_tenant(
        42,
        "garage-device-pairing",
        gateway.consume_pairing_code("one-time-code".into(), now),
    )
    .await
    .expect("consume query succeeds");

    assert!(
        consumed.is_none(),
        "expired, revoked or already consumed codes lose the compare-and-set"
    );
}
