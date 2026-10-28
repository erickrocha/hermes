//! `EPIC-SC-01-S04`: the customer gateway's reads -- by id, by uuid, by taxpayer document inside the
//! tenant, and the paged search that also matches the legal name and the document.

use business::commons::entity_mapper::EntityMapper;
use business::domain::customer::{Customer, CustomerEntityMapper};
use business::gateway::customer_gateway::{CustomerGateway, CustomerStore};
use chrono::{TimeZone, Utc};
use entity::audit::{AuditUser, run_with_user};
use entity::customer_entity;
use sea_orm::{DatabaseBackend, MockDatabase, MockExecResult, Transaction, Value};
use std::collections::BTreeMap;

fn count(value: i64) -> Vec<Vec<BTreeMap<&'static str, Value>>> {
    vec![vec![BTreeMap::from([("num_items", Value::BigInt(Some(value)))])]]
}

fn row(id: i64, tax_id: Option<&str>) -> customer_entity::Model {
    customer_entity::Model {
        id,
        uuid: vec![id as u8; 16],
        tenant_id: Some(42),
        name: "Acme".into(),
        legal_name: Some("Acme Ltda".into()),
        tax_id: tax_id.map(str::to_string),
        address_line1: Some("Rua A, 10".into()),
        address_line2: None,
        locality: Some("Campinas".into()),
        administrative_area: Some("SP".into()),
        postal_code: Some("13000000".into()),
        country_code: Some("BR".into()),
        status: "Active".into(),
        notes: None,
        created_at: Utc.with_ymd_and_hms(2026, 10, 28, 12, 0, 0).unwrap(),
        created_by: None,
        updated_at: Utc.with_ymd_and_hms(2026, 10, 28, 12, 0, 0).unwrap(),
        updated_by: None,
    }
}

#[tokio::test]
async fn customer_gateway_reads_by_id_uuid_and_document_and_searches_the_page() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([vec![row(1, Some("11222333000181"))]])
        .append_query_results([Vec::<customer_entity::Model>::new()])
        .append_query_results([vec![row(1, Some("11222333000181"))]])
        .append_query_results(count(1))
        .append_query_results([vec![row(1, Some("11222333000181"))]])
        .append_exec_results([MockExecResult { last_insert_id: 2, rows_affected: 1 }])
        .append_query_results([vec![row(2, None)]])
        .into_connection();
    let customers = CustomerGateway::new(db.clone());
    let user = AuditUser { id: 1, email: "owner@example.com".into(), tenant_id: Some(42), enforce_tenant: true };

    let (by_id, by_uuid, by_document, (page, total), saved) = run_with_user(Some(user), async {
        Ok::<_, business::domain::business_error::BusinessError>((
            customers.get_by_id(1).await?,
            customers.get_by_uuid("ff".repeat(16)).await?,
            customers.get_by_tax_id(Some(42), "11222333000181").await?,
            customers.list_page(0, 10, Some("Ltda")).await?,
            customers.save(Customer { id: None, uuid: None, ..CustomerEntityMapper::from_model(row(2, None)) }).await?,
        ))
    })
    .await
    .unwrap();

    let found = by_id.expect("the tenant's customer");
    assert_eq!((found.tax_id.as_deref(), found.legal_name.as_deref(), found.postal_code.as_deref()), (Some("11222333000181"), Some("Acme Ltda"), Some("13000000")));
    assert!(by_uuid.is_none());
    assert_eq!(by_document.expect("holder").id, Some(1));
    assert_eq!((page.len(), total), (1, 1));
    assert_eq!(saved.name, "Acme");

    let log = db.into_transaction_log();
    let search = log.iter().filter_map(|t: &Transaction| {
        let sql = format!("{t:?}");
        sql.contains("legal_name` LIKE").then_some(sql)
    });
    assert!(search.count() >= 1, "the search also matches the legal name and the document");
}
