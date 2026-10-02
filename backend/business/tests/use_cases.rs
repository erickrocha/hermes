//! DEF-XF-10: the use cases against `sea_orm::MockDatabase` — the business
//! rules exercised through the real gateways, with no database. The mock replays
//! results in the order the code asks for them, so each test also pins down
//! *which* queries a use case makes (and, for rejections, that it makes none).
//!
//! Run: cargo test -p business --features mock --test use_cases

use business::commons::functions::string_to_bytes;
use business::domain::business_plan::BusinessPlan;
use business::domain::city::City;
use business::domain::enums::Role;
use business::domain::province::Province;
use business::domain::tenant::Tenant;
use business::domain::user::User;
use business::gateway::business_plan_gateway::BusinessPlanGateway;
use business::gateway::city_gateway::CityGateway;
use business::gateway::province_gateway::ProvinceGateway;
use business::gateway::tenant_gateway::TenantGateway;
use business::gateway::user_gateway::UserGateway;
use business::use_cases::business_plan_use_case::BusinessPlanUseCase;
use business::use_cases::city_use_case::CityUseCase;
use business::use_cases::province_use_case::ProvinceUseCase;
use business::use_cases::tenant_use_case::TenantUseCase;
use business::use_cases::user_use_case::UserUseCase;
use chrono::{NaiveDate, TimeZone, Utc};
use entity::audit::{AuditUser, run_as_platform, run_with_user};
use entity::{
    business_plan_entity, city_entity, province_entity, schedule_exception_entity, tenant_entity,
    user_entity,
};
use sea_orm::{DatabaseBackend, DatabaseConnection, MockDatabase, MockExecResult, Transaction};

const UUID: &str = "684db325-63cb-470a-aa09-45811dd1904a";

fn at() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 18, 12, 0, 0).unwrap()
}

fn mock() -> MockDatabase {
    MockDatabase::new(DatabaseBackend::MySql)
}

fn inserted(id: u64) -> MockExecResult {
    MockExecResult {
        last_insert_id: id,
        rows_affected: 1,
    }
}

fn log(db: DatabaseConnection) -> Vec<Transaction> {
    db.into_transaction_log()
}

fn owner(tenant_id: i64) -> AuditUser {
    AuditUser {
        id: 2,
        email: "owner@example.com".into(),
        tenant_id: Some(tenant_id),
        enforce_tenant: true,
    }
}

// ================================================================ models

fn province_row(id: i64, acronym: &str, country: &str) -> province_entity::Model {
    province_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        acronym: acronym.into(),
        name: format!("Province {acronym}"),
        country_code: country.into(),
    }
}

fn city_row(id: i64, province_id: i64, name: &str) -> city_entity::Model {
    city_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        province_id,
        name: name.into(),
    }
}

fn plan_row(id: i64) -> business_plan_entity::Model {
    business_plan_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        name: "Professional".into(),
        price_in_cents: 10_000,
        available_users: 10,
        period_days: 30,
        payment_date: NaiveDate::from_ymd_opt(2026, 9, 30).unwrap(),
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

fn tenant_row(id: i64) -> tenant_entity::Model {
    tenant_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        business_name: "Transportes LTDA".into(),
        company_name: Some("Transmega".into()),
        tax_id: "11222333000181".into(),
        email: None,
        phone: None,
        website: None,
        address_line1: None,
        address_line2: None,
        locality: None,
        administrative_area: None,
        postal_code: None,
        country_code: Some("BR".into()),
        business_plan_id: None,
        created_at: at(),
        created_by: Some("admin".into()),
        updated_at: at(),
        updated_by: None,
    }
}

fn user_row(
    id: i64,
    email: &str,
    role: &str,
    tenant_id: Option<i64>,
    password: &str,
) -> user_entity::Model {
    user_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        name: Some("Someone".into()),
        email: email.into(),
        password: password.into(),
        enabled: true,
        blocked_reason: None,
        tenant_id,
        role: role.into(),
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

// ================================================================ provinces

fn province(acronym: &str, name: &str, country: &str) -> Province {
    Province {
        id: None,
        uuid: None,
        acronym: acronym.into(),
        name: name.into(),
        country_code: country.into(),
    }
}

#[tokio::test]
async fn saving_a_new_province_normalises_it_and_inserts_once() {
    let db = mock()
        .append_query_results([Vec::<province_entity::Model>::new()]) // no existing acronym
        .append_exec_results([inserted(80)])
        .append_query_results([[province_row(80, "ZZ", "BR")]])
        .into_connection();
    let saved = ProvinceUseCase::new(ProvinceGateway::new(db.clone()))
        .save(province(" zz ", " Testlandia ", "br"))
        .await
        .expect("valid province saves");
    assert_eq!(saved.id, Some(80));
    let sql = format!("{:?}", log(db));
    assert!(
        sql.contains(r#"String(Some("ZZ"))"#) && sql.contains(r#"String(Some("BR"))"#),
        "stored values must be normalised: {sql}"
    );
    assert!(
        !sql.contains(r#"Some(" zz ")"#) && !sql.contains(r#"Some("br")"#),
        "raw CSV values leaked: {sql}"
    );
}

#[tokio::test]
async fn saving_a_duplicate_acronym_is_refused_before_writing() {
    let db = mock()
        .append_query_results([[province_row(26, "SP", "BR")]])
        .into_connection();
    let err = ProvinceUseCase::new(ProvinceGateway::new(db.clone()))
        .save(province("SP", "Another São Paulo", "BR"))
        .await
        .expect_err("a second SP in BR must be refused");
    assert!(err.english().contains("already exists"));
    assert_eq!(log(db).len(), 1, "only the lookup ran; nothing was written");
}

#[tokio::test]
async fn an_invalid_province_never_reaches_the_database() {
    let db = mock().into_connection();
    let err = ProvinceUseCase::new(ProvinceGateway::new(db.clone()))
        .save(province("SP", "", "BR"))
        .await
        .expect_err("blank name");
    assert!(err.english().contains("name is required"));
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn province_import_creates_new_rows_and_updates_existing_ones() {
    let db = mock()
        .append_query_results([Vec::<province_entity::Model>::new()]) // ZZ: new
        .append_exec_results([inserted(81)])
        .append_query_results([[province_row(81, "ZZ", "BR")]])
        .append_query_results([[province_row(26, "SP", "BR")]]) // SP: exists
        .append_exec_results([inserted(26)])
        .append_query_results([[province_row(26, "SP", "BR")]])
        .into_connection();
    let outcome = ProvinceUseCase::new(ProvinceGateway::new(db))
        .import(vec![
            province("zz", "Testlandia", "br"),
            province("SP", "São Paulo", "BR"),
        ])
        .await
        .expect("valid batch imports");
    assert_eq!((outcome.created, outcome.updated), (1, 1));
}

#[tokio::test]
async fn province_import_is_all_or_nothing() {
    let db = mock().into_connection();
    let err = ProvinceUseCase::new(ProvinceGateway::new(db.clone()))
        .import(vec![
            province("ZZ", "Good", "BR"),
            province("YY", "", "BR"),
            province("ZZ", "Duplicate of row 1", "BR"),
        ])
        .await
        .expect_err("one bad row rejects the batch");
    let message = err.english();
    assert!(message.contains("row 2: name is required"), "{message}");
    assert!(message.contains("row 3: duplicate acronym"), "{message}");
    assert!(
        !message.contains("row 2: duplicate"),
        "the duplicate is row 3, not row 2: {message}"
    );
    assert!(
        log(db).is_empty(),
        "a rejected batch must not touch the database"
    );
}

/// DEF-RD-02: the unique index behind this check is accent- and
/// case-insensitive, so "SP" and "sp" are one province. The in-file check used
/// an exact string key and let both through; the second then silently
/// overwrote the first and the file reported as fully imported.
#[tokio::test]
async fn province_import_treats_case_as_the_database_does() {
    let db = mock().into_connection();
    let err = ProvinceUseCase::new(ProvinceGateway::new(db.clone()))
        .import(vec![
            province("sp", "São Paulo", "BR"),
            province("SP", "Sao Paulo", "br"),
        ])
        .await
        .expect_err("one province, written twice");
    assert!(
        err.english().contains("row 2: duplicate acronym"),
        "{}",
        err.english()
    );
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn province_reads() {
    let db = mock()
        .append_query_results([[province_row(26, "SP", "BR")]])
        .append_query_results([Vec::<province_entity::Model>::new()])
        .append_query_results([[province_row(26, "SP", "BR")]])
        .append_query_results([vec![
            province_row(26, "SP", "BR"),
            province_row(27, "RJ", "BR"),
        ]])
        .into_connection();
    let uc = ProvinceUseCase::new(ProvinceGateway::new(db));
    assert_eq!(uc.find_by_id(26).await.unwrap().acronym, "SP");
    assert!(
        uc.find_by_id(999).await.is_err(),
        "a missing id is an error, not a default"
    );
    assert_eq!(uc.find_by_uuid(UUID.into()).await.unwrap().id, Some(26));
    assert_eq!(uc.find_by_country_code("BR".into()).await.unwrap().len(), 2);
}

// ================================================================ cities

fn city(name: &str, province_id: i64) -> City {
    City {
        id: None,
        uuid: None,
        province_id,
        name: name.into(),
    }
}

#[tokio::test]
async fn city_save_and_its_guards() {
    let db = mock()
        .append_query_results([[province_row(26, "SP", "BR")]]) // the province exists
        .append_query_results([Vec::<city_entity::Model>::new()])
        .append_exec_results([inserted(9)])
        .append_query_results([[city_row(9, 26, "Limeira")]])
        .append_query_results([[province_row(26, "SP", "BR")]]) // 2nd save re-checks
        .append_query_results([[city_row(9, 26, "Limeira")]]) // duplicate check for 2nd save
        .into_connection();
    let uc = CityUseCase::new(CityGateway::new(db));
    assert_eq!(uc.save(city(" Limeira ", 26)).await.unwrap().id, Some(9));
    let dup = uc
        .save(city("Limeira", 26))
        .await
        .expect_err("same name, same province");
    assert!(dup.english().contains("already exists"));
    assert!(
        uc.save(city("Orphan", 0)).await.is_err(),
        "a city needs a province"
    );
}

/// DEF-RD-03: an unknown `provinceId` is the operator's mistake, and it is
/// named as one. It used to reach them as MySQL error 1452 quoting a
/// constraint name — after the rows before it had already been committed.
#[tokio::test]
async fn an_unknown_province_is_named_not_reported_as_a_constraint() {
    let db = mock()
        .append_query_results([Vec::<province_entity::Model>::new()])
        .into_connection();
    let err = CityUseCase::new(CityGateway::new(db.clone()))
        .import(vec![city("Limeira", 999)])
        .await
        .expect_err("province 999 does not exist");
    let message = err.english();
    assert!(
        message.contains("row 1: province does not exist"),
        "{message}"
    );
    assert!(!message.contains("1452"), "{message}");
    assert!(!message.contains("constraint"), "{message}");
    assert_eq!(
        log(db).len(),
        1,
        "the existence check ran; nothing was written"
    );
}

/// DEF-RD-02: MySQL's collation already treated these as one city, so the
/// second spelling overwrote the first and the file reported "1 created, 1
/// updated" for what the operator sent as two new cities.
#[tokio::test]
async fn accents_and_case_do_not_make_a_second_city() {
    let db = mock().into_connection();
    let err = CityUseCase::new(CityGateway::new(db.clone()))
        .import(vec![city("São Paulo", 26), city("sao paulo", 26)])
        .await
        .expect_err("one city, spelled twice");
    assert!(
        err.english().contains("row 2: duplicate city"),
        "{}",
        err.english()
    );
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn city_import_is_all_or_nothing_and_idempotent_by_name() {
    let empty = mock().into_connection();
    let err = CityUseCase::new(CityGateway::new(empty.clone()))
        .import(vec![city("A", 26), city("A", 26)])
        .await
        .expect_err("duplicate in file");
    assert!(
        err.english().contains("row 2: duplicate city"),
        "{}",
        err.english()
    );
    assert!(log(empty).is_empty());

    let db = mock()
        .append_query_results([[province_row(26, "SP", "BR")]])
        .append_query_results([[city_row(9, 26, "Limeira")]])
        .append_exec_results([inserted(9)])
        .append_query_results([[city_row(9, 26, "Limeira")]])
        .into_connection();
    let outcome = CityUseCase::new(CityGateway::new(db))
        .import(vec![city("Limeira", 26)])
        .await
        .unwrap();
    assert_eq!(
        (outcome.created, outcome.updated),
        (0, 1),
        "re-importing updates, never duplicates"
    );
}

#[tokio::test]
async fn city_reads() {
    let db = mock()
        .append_query_results([[city_row(9, 26, "Limeira")]])
        .append_query_results([[city_row(9, 26, "Limeira")]])
        .append_query_results([vec![
            city_row(9, 26, "Limeira"),
            city_row(10, 26, "Campinas"),
        ]])
        .append_query_results([vec![city_row(9, 26, "Limeira")]])
        .append_query_results([Vec::<city_entity::Model>::new()])
        .into_connection();
    let uc = CityUseCase::new(CityGateway::new(db));
    assert_eq!(uc.find_by_id(9).await.unwrap().name, "Limeira");
    assert_eq!(uc.find_by_uuid(UUID.into()).await.unwrap().id, Some(9));
    assert_eq!(uc.find_all().await.unwrap().len(), 2);
    assert_eq!(uc.find_by_province_id(26).await.unwrap().len(), 1);
    assert!(uc.find_by_id(404).await.is_err());
}

// ================================================================ business plans

fn plan(name: &str, price: i64) -> BusinessPlan {
    BusinessPlan {
        id: None,
        uuid: None,
        name: name.into(),
        price_in_cents: price,
        available_users: 10,
        period_days: 30,
        payment_date: NaiveDate::from_ymd_opt(2026, 9, 30).unwrap(),
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn business_plan_lifecycle() {
    let db = mock()
        .append_exec_results([inserted(3)])
        .append_query_results([[plan_row(3)]]) // create
        .append_query_results([[plan_row(3)]]) // find_by_id
        .append_query_results([[plan_row(3)]]) // find_by_uuid
        .append_query_results([vec![plan_row(3), plan_row(4)]]) // find_all
        .append_query_results([[plan_row(3)]]) // update: existence check
        .append_exec_results([inserted(3)])
        .append_query_results([[plan_row(3)]]) // update: save
        .into_connection();
    let uc = BusinessPlanUseCase::new(BusinessPlanGateway::new(db));
    assert_eq!(
        uc.create(plan(" Professional ", 10_000)).await.unwrap().id,
        Some(3)
    );
    assert_eq!(uc.find_by_id(3).await.unwrap().name, "Professional");
    assert_eq!(uc.find_by_uuid(UUID).await.unwrap().id, Some(3));
    assert_eq!(uc.find_all().await.unwrap().len(), 2);
    assert_eq!(
        uc.update(3, plan("Professional", 12_000)).await.unwrap().id,
        Some(3)
    );
}

#[tokio::test]
async fn business_plan_rules_are_checked_before_any_write() {
    let db = mock().into_connection();
    let uc = BusinessPlanUseCase::new(BusinessPlanGateway::new(db.clone()));
    assert!(uc.create(plan("", 100)).await.is_err(), "name required");
    assert!(
        uc.create(plan("Paid", -1)).await.is_err(),
        "negative price refused"
    );
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn a_plan_in_use_cannot_be_deleted() {
    let db = mock()
        .append_query_results([[plan_row(3)]])
        .append_query_results([[tenant_row(1)]]) // a tenant references it
        .into_connection();
    let err = BusinessPlanUseCase::new(BusinessPlanGateway::new(db))
        .delete(3)
        .await
        .expect_err("in use");
    assert!(
        err.message.to_lowercase().contains("assigned"),
        "{}",
        err.message
    );
}

#[tokio::test]
async fn an_unused_plan_is_deleted_and_a_missing_one_is_reported() {
    let db = mock()
        .append_query_results([[plan_row(3)]])
        .append_query_results([Vec::<tenant_entity::Model>::new()])
        .append_exec_results([MockExecResult {
            last_insert_id: 0,
            rows_affected: 1,
        }])
        .append_query_results([Vec::<business_plan_entity::Model>::new()])
        .into_connection();
    let uc = BusinessPlanUseCase::new(BusinessPlanGateway::new(db));
    uc.delete(3).await.expect("unused plan deletes");
    assert!(uc.delete(404).await.is_err());
}

// ================================================================ tenants

fn tenant(
    business_name: &str,
    company: Option<&str>,
    tax_id: &str,
    country: Option<&str>,
) -> Tenant {
    Tenant {
        id: None,
        uuid: None,
        business_name: business_name.into(),
        company_name: company.map(Into::into),
        tax_id: tax_id.into(),
        email: None,
        phone: None,
        website: None,
        address_line1: None,
        address_line2: None,
        locality: None,
        administrative_area: None,
        postal_code: None,
        country_code: country.map(Into::into),
        business_plan_id: Some(99), // must be ignored: plans are set only via set_plan
        created_at: None,
        updated_at: None,
        created_by: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn creating_a_tenant_normalises_the_tax_id_and_ignores_a_supplied_plan() {
    let db = mock()
        .append_exec_results([inserted(1)])
        .append_query_results([[tenant_row(1)]])
        .into_connection();
    let created = TenantUseCase::new(TenantGateway::new(db.clone()))
        .create(tenant(
            "Transportes LTDA",
            Some("Transmega"),
            "11.222.333/0001-81",
            Some("BR"),
        ))
        .await
        .expect("valid tenant");
    assert_eq!(created.id, Some(1));
    let sql = format!("{:?}", log(db));
    assert!(
        sql.contains("11222333000181"),
        "CNPJ must be stored as digits: {sql}"
    );
    assert!(
        !sql.contains("Some(99)"),
        "a caller-supplied plan must never be written: {sql}"
    );
}

#[tokio::test]
async fn tenant_creation_rules() {
    let db = mock().into_connection();
    let uc = TenantUseCase::new(TenantGateway::new(db.clone()));
    assert!(
        uc.create(tenant("", None, "11222333000181", Some("BR")))
            .await
            .is_err(),
        "HRM-002"
    );
    assert!(
        uc.create(tenant("   ", None, "11222333000181", Some("BR")))
            .await
            .is_err(),
        "DEF-TP-03: a name of blanks"
    );
    assert!(
        uc.create(tenant("X", None, "11222333000181", Some("BRA")))
            .await
            .is_err(),
        "HRM-003"
    );
    assert!(
        uc.create(tenant("X", None, "11222333000181", None))
            .await
            .is_err(),
        "country required"
    );
    assert!(
        uc.create(tenant("X", None, "123", Some("BR")))
            .await
            .is_err(),
        "invalid CNPJ"
    );
    // DEF-TP-01: a country with no validator must still require an identifier.
    assert!(
        uc.create(tenant("X", None, "", Some("US"))).await.is_err(),
        "tax id required"
    );
    assert!(
        uc.create(tenant("X", None, "   ", Some("CA")))
            .await
            .is_err(),
        "blank tax id"
    );
    assert!(uc.persist(tenant("", None, "", Some("BR"))).await.is_none());
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn tenant_update_and_plan_rules() {
    let db = mock()
        .append_query_results([Vec::<tenant_entity::Model>::new()]) // update: not found
        .append_query_results([[tenant_row(1)]]) // update: found, business name is blank
        .append_query_results([[tenant_row(1)]]) // update: found, tax id is blank
        .append_query_results([[tenant_row(1)]]) // update: found, no company name -> allowed
        .append_exec_results([inserted(1)])
        .append_query_results([[tenant_row(1)]])
        .append_query_results([Vec::<tenant_entity::Model>::new()]) // set_plan: tenant not found
        .append_query_results([[tenant_row(1)]]) // set_plan: found
        .append_exec_results([inserted(1)])
        .append_query_results([[tenant_row(1)]])
        .into_connection();
    let uc = TenantUseCase::new(TenantGateway::new(db));

    let missing = uc
        .update(404, tenant("X", Some("Y"), "11222333000181", Some("BR")))
        .await;
    assert!(
        missing.as_ref().is_err_and(|e| e.is_not_found()),
        "an unknown tenant is a not-found, not a generic failure"
    );

    // DEF-TP-03: the business name is validated on update too, and blanking it
    // out is refused.
    assert!(
        uc.update(1, tenant("  ", Some("Y"), "11222333000181", Some("BR")))
            .await
            .is_err()
    );
    // DEF-TP-01: and so is emptying the tax identifier.
    assert!(
        uc.update(1, tenant("X", Some("Y"), "", Some("BR")))
            .await
            .is_err()
    );
    // DEF-TP-02: company name is optional on create, so it cannot be mandatory
    // on update -- a tenant onboarded with only its required fields must stay
    // editable.
    assert!(
        uc.update(1, tenant("X", None, "11222333000181", Some("BR")))
            .await
            .is_ok(),
        "a tenant without a company name must be editable"
    );

    // DEF-TP-05: setting a plan on a tenant that does not exist is a not-found.
    assert!(uc.set_plan(404, 3).await.is_err_and(|e| e.is_not_found()));
    assert!(uc.set_plan(1, 3).await.is_ok());
}

#[tokio::test]
async fn updating_a_tenant_keeps_its_plan_and_identity() {
    let mut existing = tenant_row(1);
    existing.business_plan_id = Some(3);
    let db = mock()
        .append_query_results([[existing.clone()]])
        .append_exec_results([inserted(1)])
        .append_query_results([[existing]])
        .into_connection();
    let updated = TenantUseCase::new(TenantGateway::new(db.clone()))
        .update(
            1,
            tenant(
                "Transportes LTDA",
                Some("Renamed"),
                "11222333000181",
                Some("BR"),
            ),
        )
        .await
        .expect("valid update");
    assert_eq!(updated.business_plan_id, Some(3));
    let sql = format!("{:?}", log(db));
    assert!(sql.contains("Renamed"), "{sql}");
    assert!(
        !sql.contains("Some(99)"),
        "the plan from the request body must be ignored: {sql}"
    );
}

#[tokio::test]
async fn tenant_reads() {
    let db = mock()
        .append_query_results([[tenant_row(1)]])
        .append_query_results([Vec::<tenant_entity::Model>::new()])
        .append_query_results([[tenant_row(1)]])
        .append_query_results([vec![tenant_row(1), tenant_row(2)]])
        .into_connection();
    let uc = TenantUseCase::new(TenantGateway::new(db));
    assert_eq!(
        uc.find_by_id(1).await.unwrap().company_name.as_deref(),
        Some("Transmega")
    );
    assert!(uc.find_by_id(404).await.is_err());
    assert_eq!(uc.find_by_uuid(UUID.into()).await.unwrap().id, Some(1));
    assert_eq!(uc.find_all().await.unwrap().len(), 2);
}

// ================================================================ users

fn new_user(email: &str, password: &str, role: Role, tenant_id: Option<i64>) -> User {
    User {
        id: None,
        uuid: None,
        email: email.into(),
        name: Some("New".into()),
        password: password.into(),
        enabled: true,
        tenant_id,
        role,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn creating_a_user_stores_an_argon2id_hash_never_the_password() {
    let db = mock()
        .append_exec_results([inserted(5)])
        .append_query_results([[user_row(
            5,
            "new@example.com",
            "TenantUser",
            Some(42),
            "$argon2id$x",
        )]])
        .into_connection();
    let created = run_with_user(
        Some(owner(42)),
        UserUseCase::new(UserGateway::new(db.clone())).create(new_user(
            "new@example.com",
            "Correct#Horse9",
            Role::TenantUser,
            Some(42),
        )),
    )
    .await
    .expect("valid user");
    assert_eq!(created.id, Some(5));
    let sql = format!("{:?}", log(db));
    assert!(
        !sql.contains("Correct#Horse9"),
        "plaintext password reached the database: {sql}"
    );
    assert!(
        sql.contains("$argon2id$"),
        "password must be stored as Argon2id: {sql}"
    );
}

#[tokio::test]
async fn user_creation_rules_are_checked_before_any_write() {
    let db = mock().into_connection();
    let uc = UserUseCase::new(UserGateway::new(db.clone()));
    assert!(
        uc.create(new_user("", "Correct#Horse9", Role::TenantUser, Some(42)))
            .await
            .is_err()
    );
    assert!(
        uc.create(new_user("a@b.c", "", Role::TenantUser, Some(42)))
            .await
            .is_err()
    );
    assert!(
        uc.create(new_user("a@b.c", "short", Role::TenantUser, Some(42)))
            .await
            .is_err()
    );
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn a_user_write_without_any_tenant_scope_is_refused() {
    // D-05/D-06: outside a request the scope is Denied, and Denied refuses the
    // write instead of storing a tenant-less row.
    let db = mock().into_connection();
    let err = UserUseCase::new(UserGateway::new(db))
        .create(new_user(
            "new@example.com",
            "Correct#Horse9",
            Role::TenantUser,
            Some(42),
        ))
        .await
        .expect_err("no scope, no write");
    assert!(!err.message.is_empty());
}

#[tokio::test]
async fn updating_a_user_without_a_password_keeps_the_stored_hash() {
    let existing = user_row(
        5,
        "u@example.com",
        "TenantUser",
        Some(42),
        "$argon2id$stored",
    );
    let db = mock()
        .append_query_results([[existing.clone()]])
        .append_exec_results([inserted(5)])
        .append_query_results([[existing]])
        .into_connection();
    run_with_user(
        Some(owner(42)),
        UserUseCase::new(UserGateway::new(db.clone()))
            .update(5, new_user("u@example.com", "", Role::TenantUser, Some(42))),
    )
    .await
    .expect("update without password");
    let sql = format!("{:?}", log(db));
    assert!(
        sql.contains("$argon2id$stored"),
        "HRM-035: an omitted password keeps the old one: {sql}"
    );
}

#[tokio::test]
async fn changing_a_password_requires_the_current_one() {
    let current = business::commons::password::hash("Current#Pass1").unwrap();
    let row = user_row(5, "u@example.com", "TenantUser", Some(42), &current);
    let db = mock()
        .append_query_results([[row.clone()]]) // wrong current password
        .append_query_results([[row.clone()]]) // right current password
        .append_exec_results([inserted(5)])
        .append_query_results([[row]])
        .into_connection();
    let uc = UserUseCase::new(UserGateway::new(db));
    let wrong = run_with_user(
        Some(owner(42)),
        uc.change_password(5, "nope".into(), "Brand#New99".into()),
    )
    .await;
    assert!(wrong.is_err(), "wrong current password must be refused");
    let ok = run_with_user(
        Some(owner(42)),
        uc.change_password(5, "Current#Pass1".into(), "Brand#New99".into()),
    )
    .await;
    assert!(ok.is_ok(), "{:?}", ok.err().map(|e| e.message));
    assert!(
        uc.change_password(5, "x".into(), "short".into())
            .await
            .is_err(),
        "policy applies to the new one"
    );
}

#[tokio::test]
async fn user_reads() {
    let row = user_row(5, "u@example.com", "TenantUser", Some(42), "h");
    let db = mock()
        .append_query_results([[row.clone()]])
        .append_query_results([Vec::<user_entity::Model>::new()])
        .append_query_results([vec![row.clone(), row.clone()]])
        .append_query_results([vec![row.clone()]])
        .append_query_results([[row]])
        .into_connection();
    let uc = UserUseCase::new(UserGateway::new(db.clone()));
    assert_eq!(
        run_with_user(Some(owner(42)), uc.find_by_id(5))
            .await
            .unwrap()
            .email,
        "u@example.com"
    );
    assert!(
        run_with_user(Some(owner(42)), uc.find_by_id(404))
            .await
            .is_err()
    );
    assert_eq!(
        run_with_user(Some(owner(42)), uc.find_all())
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(uc.find_all_by_tenant_id(42).await.unwrap().len(), 1);
    assert!(
        UserUseCase::find_by_email(&db, "u@example.com".into())
            .await
            .is_some()
    );
}

#[tokio::test]
async fn seeding_creates_the_sysadmin_once_under_platform_scope() {
    // SAFETY: tests in this binary never read these two variables elsewhere.
    unsafe {
        std::env::set_var("SYSADMIN_EMAIL", "admin@hermes.test");
        std::env::set_var("SYSADMIN_PASSWORD", "Seeded#Admin1");
    }
    let db = mock()
        .append_query_results([Vec::<user_entity::Model>::new()]) // no sysadmin yet
        .append_exec_results([inserted(1)])
        .append_query_results([[user_row(
            1,
            "admin@hermes.test",
            "SysAdmin",
            None,
            "$argon2id$x",
        )]])
        .append_query_results([[user_row(
            1,
            "admin@hermes.test",
            "SysAdmin",
            None,
            "$argon2id$x",
        )]]) // second boot
        .into_connection();
    let first = run_as_platform(UserUseCase::seed_sysadmin(&db))
        .await
        .expect("seeded");
    assert_eq!(first.role, Role::SysAdmin);
    assert_eq!(first.tenant_id, None);
    let again = run_as_platform(UserUseCase::seed_sysadmin(&db))
        .await
        .expect("found");
    assert_eq!(
        again.id,
        Some(1),
        "a second boot must not create a second administrator"
    );
}

// ================================================================ paging (PD-028)

use sea_orm::{DbErr, Value};
use std::collections::BTreeMap;

/// The paginator asks for the total first, then the page.
fn count(n: i64) -> Vec<Vec<BTreeMap<&'static str, Value>>> {
    vec![vec![BTreeMap::from([(
        "num_items",
        Value::BigInt(Some(n)),
    )])]]
}

#[tokio::test]
async fn every_list_returns_its_page_and_the_table_total() {
    let db = mock()
        .append_query_results(count(51))
        .append_query_results([vec![tenant_row(1), tenant_row(2)]])
        .append_query_results(count(3))
        .append_query_results([vec![plan_row(3)]])
        .append_query_results(count(78))
        .append_query_results([vec![province_row(26, "SP", "BR")]])
        .append_query_results(count(5000))
        .append_query_results([vec![city_row(9, 26, "Limeira")]])
        .append_query_results(count(2))
        .append_query_results([vec![user_row(
            5,
            "u@example.com",
            "TenantUser",
            Some(42),
            "h",
        )]])
        .into_connection();

    let (tenants, total) = TenantUseCase::new(TenantGateway::new(db.clone()))
        .find_page(0, 25, Some("Trans"))
        .await
        .unwrap();
    assert_eq!((tenants.len(), total), (2, 51));
    let (plans, total) = BusinessPlanUseCase::new(BusinessPlanGateway::new(db.clone()))
        .find_page(0, 25, Some("Pro"))
        .await
        .unwrap();
    assert_eq!((plans.len(), total), (1, 3));
    let (provinces, total) = ProvinceUseCase::new(ProvinceGateway::new(db.clone()))
        .find_page(0, 25, Some("SP"))
        .await
        .unwrap();
    assert_eq!((provinces.len(), total), (1, 78));
    let (cities, total) = CityUseCase::new(CityGateway::new(db.clone()))
        .find_page(3, 25, Some("Lim"))
        .await
        .unwrap();
    assert_eq!((cities.len(), total), (1, 5000));
    let (users, total) = run_with_user(
        Some(owner(42)),
        UserUseCase::new(UserGateway::new(db.clone())).find_page(0, 25, Some("u@")),
    )
    .await
    .unwrap();
    assert_eq!((users.len(), total), (1, 2));

    let sql = format!("{:?}", log(db));
    assert!(
        sql.contains("LIKE"),
        "a search term must reach the database as a filter: {sql}"
    );
    assert!(
        sql.contains("OFFSET"),
        "page 3 must be an OFFSET query, not a client-side slice: {sql}"
    );
}

#[tokio::test]
async fn a_tenant_owners_user_page_is_filtered_to_their_tenant() {
    let db = mock()
        .append_query_results(count(0))
        .append_query_results([Vec::<user_entity::Model>::new()])
        .into_connection();
    run_with_user(
        Some(owner(42)),
        UserUseCase::new(UserGateway::new(db.clone())).find_page(0, 25, None),
    )
    .await
    .unwrap();
    let sql = format!("{:?}", log(db));
    assert!(
        sql.contains("tenant_id") && sql.contains("42"),
        "paging must not escape tenant scope: {sql}"
    );
}

// ================================================================ database failures

fn failing(times: usize) -> DatabaseConnection {
    mock()
        .append_query_errors((0..times).map(|_| DbErr::Custom("database unavailable".into())))
        .into_connection()
}

#[tokio::test]
async fn database_failures_surface_as_errors_not_as_empty_results() {
    // DEF-XF-02's class of bug: a failure must never look like "nothing found".
    let tenants = TenantUseCase::new(TenantGateway::new(failing(4)));
    assert!(tenants.find_by_id(1).await.is_err());
    assert!(tenants.find_by_uuid(UUID.into()).await.is_err());
    assert!(tenants.find_all().await.is_err());
    assert!(tenants.find_page(0, 25, None).await.is_err());

    let plans = BusinessPlanUseCase::new(BusinessPlanGateway::new(failing(4)));
    assert!(plans.find_by_id(1).await.is_err());
    assert!(plans.find_by_uuid(UUID).await.is_err());
    assert!(plans.find_all().await.is_err());
    assert!(plans.find_page(0, 25, None).await.is_err());

    let provinces = ProvinceUseCase::new(ProvinceGateway::new(failing(5)));
    assert!(provinces.find_by_id(1).await.is_err());
    assert!(provinces.find_by_uuid(UUID.into()).await.is_err());
    assert!(provinces.find_by_country_code("BR".into()).await.is_err());
    assert!(provinces.find_page(0, 25, None).await.is_err());
    assert!(
        provinces
            .import(vec![province("SP", "São Paulo", "BR")])
            .await
            .is_err()
    );

    let cities = CityUseCase::new(CityGateway::new(failing(6)));
    assert!(cities.find_by_id(1).await.is_err());
    assert!(cities.find_by_uuid(UUID.into()).await.is_err());
    assert!(cities.find_all().await.is_err());
    assert!(cities.find_by_province_id(26).await.is_err());
    assert!(cities.find_page(0, 25, None).await.is_err());
    assert!(cities.import(vec![city("Limeira", 26)]).await.is_err());

    let users = UserUseCase::new(UserGateway::new(failing(4)));
    assert!(
        run_with_user(Some(owner(42)), users.find_by_id(1))
            .await
            .is_err()
    );
    assert!(
        run_with_user(Some(owner(42)), users.find_all())
            .await
            .is_err()
    );
    assert!(users.find_all_by_tenant_id(42).await.is_err());
    assert!(
        run_with_user(Some(owner(42)), users.find_page(0, 25, None))
            .await
            .is_err()
    );
    assert!(
        UserUseCase::find_by_email(&failing(1), "u@example.com".into())
            .await
            .is_none()
    );
}

#[tokio::test]
async fn failed_writes_are_reported() {
    let write_fails = || {
        mock()
            .append_exec_errors([DbErr::Custom("constraint violation".into())])
            .into_connection()
    };
    let tenant_err = TenantUseCase::new(TenantGateway::new(write_fails()))
        .create(tenant(
            "Transportes LTDA",
            None,
            "11222333000181",
            Some("BR"),
        ))
        .await;
    assert!(tenant_err.is_err());

    let plan_err = BusinessPlanUseCase::new(BusinessPlanGateway::new(write_fails()))
        .create(plan("Pro", 100))
        .await;
    assert!(plan_err.is_err());

    let user_err = run_with_user(
        Some(owner(42)),
        UserUseCase::new(UserGateway::new(write_fails())).create(new_user(
            "n@example.com",
            "Correct#Horse9",
            Role::TenantUser,
            Some(42),
        )),
    )
    .await;
    assert!(user_err.is_err());

    let persisted = run_as_platform(UserUseCase::persist(
        write_fails(),
        new_user(
            "n@example.com",
            "Correct#Horse9",
            Role::TenantUser,
            Some(42),
        ),
    ))
    .await;
    assert!(persisted.is_err());
    assert!(
        UserUseCase::persist(
            mock().into_connection(),
            new_user("", "", Role::TenantUser, None)
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn persisting_a_user_hashes_the_password() {
    let db = mock()
        .append_exec_results([inserted(6)])
        .append_query_results([[user_row(
            6,
            "p@example.com",
            "TenantUser",
            Some(42),
            "$argon2id$x",
        )]])
        .into_connection();
    let saved = run_as_platform(UserUseCase::persist(
        db.clone(),
        new_user(
            "p@example.com",
            "Correct#Horse9",
            Role::TenantUser,
            Some(42),
        ),
    ))
    .await
    .expect("persisted");
    assert_eq!(saved.id, Some(6));
    assert!(!format!("{:?}", log(db)).contains("Correct#Horse9"));
}

#[tokio::test]
async fn updating_a_user_never_changes_the_password() {
    // DEF-IA-03 (HRMS-108, HRMS-018): `PUT /user/{id}` is user administration,
    // and administration never sets credentials. A `password` in the payload is
    // ignored outright -- not hashed, not validated -- and the stored hash is
    // written back untouched. Changing a password happens through invitation
    // (PD-002), which is the only path that proves possession of the account.
    let existing = user_row(5, "u@example.com", "TenantUser", Some(42), "$argon2id$old");
    let db = mock()
        .append_query_results([[existing.clone()]])
        .append_exec_results([inserted(5)])
        .append_query_results([[existing]])
        .into_connection();
    run_with_user(
        Some(owner(42)),
        UserUseCase::new(UserGateway::new(db.clone())).update(
            5,
            new_user("u@example.com", "Brand#New99", Role::TenantUser, Some(42)),
        ),
    )
    .await
    .expect("update ignores the password");
    let sql = format!("{:?}", log(db));
    assert!(
        !sql.contains("Brand#New99"),
        "a supplied password must never reach the database: {sql}"
    );
    assert!(
        sql.contains("$argon2id$old"),
        "the stored hash must be written back unchanged: {sql}"
    );

    // A password that could never be *set* does not block an edit either,
    // because the field is not a credential on this route.
    let short = run_with_user(
        Some(owner(42)),
        UserUseCase::new(UserGateway::new(
            mock()
                .append_query_results([[user_row(5, "u@example.com", "TenantUser", Some(42), "h")]])
                .append_exec_results([inserted(5)])
                .append_query_results([[user_row(5, "u@example.com", "TenantUser", Some(42), "h")]])
                .into_connection(),
        ))
        .update(
            5,
            new_user("u@example.com", "short", Role::TenantUser, Some(42)),
        ),
    )
    .await;
    assert!(short.is_ok(), "the password field is inert on update");

    assert!(
        run_with_user(
            Some(owner(42)),
            UserUseCase::new(UserGateway::new(failing(1)))
                .update(5, new_user("u@example.com", "", Role::TenantUser, Some(42)))
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn changing_sysadmin_email_repoints_the_same_account_instead_of_adding_one() {
    // SAFETY: same values as the other seeding test in this binary.
    unsafe {
        std::env::set_var("SYSADMIN_EMAIL", "admin@hermes.test");
        std::env::set_var("SYSADMIN_PASSWORD", "Seeded#Admin1");
    }
    let old = user_row(1, "old-admin@hermes.test", "SysAdmin", None, "$argon2id$x");
    let db = mock()
        .append_query_results([[old]])
        .append_exec_results([inserted(1)])
        .append_query_results([[user_row(
            1,
            "admin@hermes.test",
            "SysAdmin",
            None,
            "$argon2id$x",
        )]])
        .into_connection();
    let seeded = run_as_platform(UserUseCase::seed_sysadmin(&db))
        .await
        .expect("re-pointed");
    assert_eq!(
        seeded.id,
        Some(1),
        "the existing administrator is updated, not duplicated"
    );
    let sql = format!("{:?}", log(db));
    assert!(sql.contains("UPDATE"), "{sql}");
    assert!(!sql.contains("INSERT"), "no second administrator: {sql}");
}

// ================================================================ vehicles
//
// EPIC-FO-01 (HRMS-920...925, D-09/D-20): the vehicle register through its
// real gateway. The tenant scope is what these pin down -- the SQL the mock
// records is the evidence that every read and write carries it.

use business::domain::enums::VehicleStatus;
use business::domain::vehicle::Vehicle;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::use_cases::vehicle_use_case::{
    DUPLICATE_PLATE, DUPLICATE_TRACKER_DEVICE, VehicleUseCase,
};
use entity::vehicle_entity;

fn vehicle_row(id: i64, tenant_id: i64, plate: &str, status: &str) -> vehicle_entity::Model {
    vehicle_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        plate: plate.into(),
        model: "Volvo FH".into(),
        status: status.into(),
        tracker_device_id: None,
        prefix: None,
        vehicle_type: None,
        odometer_km: None,
        wheel_type: None,
        spare_tire_count: None,
        spare_tire_type: None,
        spare_tire_notes: None,
        tank_capacity_liters: None,
        reference_km_per_liter: None,
        garage_tag: None,
        garage_tag_origin: None,
        created_at: at(),
        created_by: Some("owner@example.com".into()),
        updated_at: at(),
        updated_by: None,
    }
}

fn new_vehicle(plate: &str, model: &str, status: VehicleStatus, tenant_id: Option<i64>) -> Vehicle {
    Vehicle {
        id: None,
        uuid: None,
        tenant_id,
        plate: plate.into(),
        model: model.into(),
        status,
        tracker_device_id: None,
        prefix: None,
        vehicle_type: None,
        odometer_km: None,
        wheel_type: None,
        spare_tire_count: None,
        spare_tire_type: None,
        spare_tire_notes: None,
        tank_capacity_liters: None,
        reference_km_per_liter: None,
        garage_tag: None,
        garage_tag_origin: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn registering_a_vehicle_normalises_its_plate_and_stamps_the_callers_tenant() {
    // HRMS-920/HRMS-925: one spelling per plate. HRMS-921/D-06: the owning
    // tenant is the caller's own, never one the payload named.
    let db = mock()
        .append_query_results([Vec::<vehicle_entity::Model>::new()]) // no duplicate plate
        .append_exec_results([inserted(10)])
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .into_connection();

    let created = run_with_user(
        Some(owner(42)),
        VehicleUseCase::new(VehicleGateway::new(db.clone())).create(new_vehicle(
            "  abc1d23 ",
            " Volvo FH ",
            VehicleStatus::Active,
            Some(999),
        )),
    )
    .await
    .expect("a tenant owner registers a vehicle");

    assert_eq!(created.plate, "ABC1D23");
    assert_eq!(created.tenant_id, Some(42));
    let sql = format!("{:?}", log(db));
    assert!(sql.contains("ABC1D23"), "{sql}");
    assert!(
        !sql.contains("999"),
        "the payload's tenant must never be written: {sql}"
    );
}

#[tokio::test]
async fn a_plate_already_registered_in_the_same_tenant_is_refused_before_any_insert() {
    // HRMS-925/D-23(c): the duplicate lookup is tenant-scoped, so it can only
    // ever report a collision inside the caller's own fleet -- a global check
    // would answer "taken" for another customer's vehicle.
    let db = mock()
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .into_connection();

    let refused = run_with_user(
        Some(owner(42)),
        VehicleUseCase::new(VehicleGateway::new(db.clone())).create(new_vehicle(
            "abc1d23",
            "Volvo FH",
            VehicleStatus::Active,
            None,
        )),
    )
    .await
    .expect_err("a duplicate plate is refused");
    assert_eq!(refused.message, DUPLICATE_PLATE);

    let sql = format!("{:?}", log(db));
    assert!(!sql.contains("INSERT"), "nothing may be written: {sql}");
    assert!(
        sql.contains("tenant_id") && sql.contains("42"),
        "the check is scoped: {sql}"
    );
}

#[tokio::test]
async fn a_tracker_device_already_linked_to_another_vehicle_is_refused() {
    // HRMS-926: one device, one vehicle, platform-wide. The platform
    // administrator's scope is unrestricted, so the lookup spans every tenant.
    let mut linked = vehicle_row(10, 42, "ABC1D23", "Active");
    linked.tracker_device_id = Some(393333);
    let db = mock()
        .append_query_results([Vec::<vehicle_entity::Model>::new()]) // plate free in tenant 7
        .append_query_results([[linked]])
        .into_connection();
    let platform = AuditUser {
        id: 1,
        email: "admin@example.com".into(),
        tenant_id: None,
        enforce_tenant: false,
    };
    let mut vehicle = new_vehicle("XYZ9A87", "Marcopolo", VehicleStatus::Active, Some(7));
    vehicle.tracker_device_id = Some(393333);

    let refused = run_with_user(
        Some(platform),
        VehicleUseCase::new(VehicleGateway::new(db.clone())).create(vehicle),
    )
    .await
    .expect_err("a linked device is refused");
    assert_eq!(refused.message, DUPLICATE_TRACKER_DEVICE);
    let sql = format!("{:?}", log(db));
    assert!(!sql.contains("INSERT"), "nothing may be written: {sql}");
}

#[tokio::test]
async fn a_vehicle_without_a_plate_or_a_model_never_reaches_the_database() {
    let blank_plate = run_with_user(
        Some(owner(42)),
        VehicleUseCase::new(VehicleGateway::new(mock().into_connection())).create(new_vehicle(
            "   ",
            "Volvo FH",
            VehicleStatus::Active,
            None,
        )),
    )
    .await;
    assert!(blank_plate.is_err());

    let blank_model = run_with_user(
        Some(owner(42)),
        VehicleUseCase::new(VehicleGateway::new(mock().into_connection())).create(new_vehicle(
            "ABC1D23",
            "  ",
            VehicleStatus::Active,
            None,
        )),
    )
    .await;
    assert!(blank_model.is_err());
}

#[tokio::test]
async fn every_vehicle_read_is_filtered_to_the_callers_tenant() {
    // HRMS-921/D-09: the read half of the rule, observed on the SQL the
    // gateway actually issues rather than on the rows it happens to return.
    let db = mock()
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Maintenance")]])
        .into_connection();
    let use_case = VehicleUseCase::new(VehicleGateway::new(db.clone()));

    let found = run_with_user(Some(owner(42)), use_case.find_by_uuid(UUID.into()))
        .await
        .unwrap();
    assert_eq!(found.status, VehicleStatus::Active);
    assert_eq!(
        run_with_user(Some(owner(42)), use_case.find_by_id(10))
            .await
            .unwrap()
            .id,
        Some(10)
    );
    assert_eq!(
        run_with_user(Some(owner(42)), use_case.find_all())
            .await
            .unwrap()
            .len(),
        1
    );

    let sql = format!("{:?}", log(db));
    assert_eq!(
        sql.matches("42").count(),
        3,
        "every read carries the tenant scope: {sql}"
    );
}

#[tokio::test]
async fn paging_vehicles_is_scoped_and_searches_plate_and_model() {
    // PD-028: paging must not become the read path that escapes the scope.
    let db = mock()
        .append_query_results(count(1))
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .into_connection();

    let (vehicles, total) = run_with_user(
        Some(owner(42)),
        VehicleUseCase::new(VehicleGateway::new(db.clone())).find_page(0, 25, Some("ABC")),
    )
    .await
    .expect("a page of the caller's own fleet");
    assert_eq!((vehicles.len(), total), (1, 1));

    let sql = format!("{:?}", log(db));
    assert!(sql.contains("tenant_id") && sql.contains("42"), "{sql}");
    assert!(sql.contains("plate"), "{sql}");
    assert!(sql.contains("model"), "{sql}");
    assert!(
        sql.contains("LIKE"),
        "the search term reaches the database: {sql}"
    );
}

#[tokio::test]
async fn a_caller_outside_any_tenant_scope_finds_no_vehicle_at_all() {
    // D-05: without a request scope the read is Denied, not unrestricted.
    let db = mock()
        .append_query_results([Vec::<vehicle_entity::Model>::new()])
        .into_connection();
    let refused = VehicleUseCase::new(VehicleGateway::new(db.clone()))
        .find_by_uuid(UUID.into())
        .await;
    assert!(refused.is_err(), "an unscoped caller reads nothing");
    assert!(format!("{:?}", log(db)).contains("1 = 0"));
}

#[tokio::test]
async fn editing_a_vehicle_cannot_move_it_to_another_tenant() {
    // HRMS-921: a vehicle never changes hands through an edit.
    let existing = vehicle_row(10, 42, "ABC1D23", "Active");
    let db = mock()
        .append_query_results([[existing.clone()]]) // find_by_id
        .append_query_results([[existing]]) // duplicate check finds itself
        .append_exec_results([inserted(10)])
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Maintenance")]])
        .into_connection();

    let updated = run_with_user(
        Some(owner(42)),
        VehicleUseCase::new(VehicleGateway::new(db.clone())).update(
            10,
            new_vehicle("abc1d23", "Volvo FH", VehicleStatus::Maintenance, Some(999)),
        ),
    )
    .await
    .expect("its own tenant owner may edit it");

    assert_eq!(updated.tenant_id, Some(42));
    assert_eq!(updated.status, VehicleStatus::Maintenance);
    let sql = format!("{:?}", log(db));
    assert!(
        !sql.contains("999"),
        "the payload's tenant must be ignored: {sql}"
    );
}

#[tokio::test]
async fn reusing_another_vehicles_plate_while_editing_is_refused() {
    let db = mock()
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]]) // find_by_id
        .append_query_results([[vehicle_row(11, 42, "XYZ4E56", "Active")]]) // the plate is another row's
        .into_connection();

    let refused = run_with_user(
        Some(owner(42)),
        VehicleUseCase::new(VehicleGateway::new(db.clone())).update(
            10,
            new_vehicle("xyz4e56", "Volvo FH", VehicleStatus::Active, None),
        ),
    )
    .await
    .expect_err("a plate already carried by another vehicle is refused");
    assert_eq!(refused.message, DUPLICATE_PLATE);
    assert!(!format!("{:?}", log(db)).contains("UPDATE"));
}

#[tokio::test]
async fn vehicle_database_failures_surface_as_errors_not_as_empty_results() {
    // DEF-XF-02's class of bug, for the fleet reads.
    let vehicles = VehicleUseCase::new(VehicleGateway::new(failing(5)));
    assert!(
        run_with_user(Some(owner(42)), vehicles.find_by_id(1))
            .await
            .is_err()
    );
    assert!(
        run_with_user(Some(owner(42)), vehicles.find_by_uuid(UUID.into()))
            .await
            .is_err()
    );
    assert!(
        run_with_user(Some(owner(42)), vehicles.find_all())
            .await
            .is_err()
    );
    assert!(
        run_with_user(Some(owner(42)), vehicles.find_page(0, 25, None))
            .await
            .is_err()
    );
    assert!(
        run_with_user(
            Some(owner(42)),
            VehicleUseCase::new(VehicleGateway::new(failing(1))).create(new_vehicle(
                "ABC1D23",
                "Volvo FH",
                VehicleStatus::Active,
                None
            ))
        )
        .await
        .is_err()
    );
}

// ---------------------------------------------------------------- EPIC-FO-03 vehicle assignment
use business::domain::vehicle::VehicleEntityMapper;
use business::gateway::vehicle_assignment_gateway::VehicleAssignmentGateway;
use business::use_cases::vehicle_assignment_use_case::{
    NOT_A_DRIVER, VEHICLE_ALREADY_ASSIGNED, VehicleAssignmentUseCase,
};
use entity::vehicle_assignment_entity;

fn assignment_use_case(db: &DatabaseConnection) -> VehicleAssignmentUseCase {
    VehicleAssignmentUseCase::new(
        VehicleAssignmentGateway::new(db.clone()),
        UserGateway::new(db.clone()),
    )
}

fn live_assignment(vehicle_id: i64, driver_id: i64) -> vehicle_assignment_entity::Model {
    vehicle_assignment_entity::Model {
        id: 1,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        vehicle_id,
        driver_id,
        started_at: at(),
        ended_at: None,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

#[tokio::test]
async fn only_an_active_driver_of_the_vehicles_own_tenant_can_be_assigned() {
    // HRMS-932: a tenant user, a driver of another tenant and a disabled
    // driver all get the same answer, and nothing is written.
    use business::commons::entity_mapper::EntityMapper;
    let vehicle = VehicleEntityMapper::from_model(vehicle_row(10, 42, "ABC1D23", "Active"));
    let mut disabled = user_row(5, "d@example.com", "Driver", Some(42), "x");
    disabled.enabled = false;
    for candidate in [
        user_row(5, "u@example.com", "TenantUser", Some(42), "x"),
        user_row(5, "d@example.com", "Driver", Some(43), "x"),
        disabled,
    ] {
        let db = mock().append_query_results([[candidate]]).into_connection();
        let refused = run_with_user(Some(owner(42)), assignment_use_case(&db).assign(&vehicle, UUID.into()))
            .await
            .expect_err("not an assignable driver");
        assert_eq!(refused.message, NOT_A_DRIVER);
        assert!(!format!("{:?}", log(db)).contains("INSERT"));
    }
}

#[tokio::test]
async fn a_vehicle_with_a_live_assignment_is_refused_a_second_one() {
    // D-23(d)/HRMS-934: end the live assignment first.
    use business::commons::entity_mapper::EntityMapper;
    let vehicle = VehicleEntityMapper::from_model(vehicle_row(10, 42, "ABC1D23", "Active"));
    let db = mock()
        .append_query_results([[user_row(6, "d2@example.com", "Driver", Some(42), "x")]])
        .append_query_results([[live_assignment(10, 5)]])
        .into_connection();
    let refused = run_with_user(Some(owner(42)), assignment_use_case(&db).assign(&vehicle, UUID.into()))
        .await
        .expect_err("second live assignment");
    assert_eq!(refused.message, VEHICLE_ALREADY_ASSIGNED);
    let sql = format!("{:?}", log(db));
    assert!(!sql.contains("INSERT"), "{sql}");
    assert!(sql.contains("`ended_at` IS NULL"), "the check looks for a live row: {sql}");
}

// ---------------------------------------------------------- EPIC-SC-02 transport demand
use business::domain::transport_demand::TransportDemand;
use business::gateway::transport_demand_gateway::TransportDemandGateway;
use business::use_cases::transport_demand_use_case::{
    NOT_A_DRIVER as DEMAND_NOT_A_DRIVER, NOT_A_TENANT_VEHICLE, TransportDemandUseCase,
};

fn demand_use_case(db: &DatabaseConnection) -> TransportDemandUseCase {
    TransportDemandUseCase::new(
        TransportDemandGateway::new(db.clone()),
        UserGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
    )
}

fn new_demand(tenant_id: Option<i64>) -> TransportDemand {
    TransportDemand {
        id: None,
        uuid: None,
        tenant_id,
        demand_type: "RecurringLine".into(),
        demand_kind: None,
        customer_id: None,
        line_name: None,
        shift_start: None,
        shift_end: None,
        days_of_week: None,
        specific_date: None,
        priority: None,
        preferred_vehicle_type: None,
        preferred_vehicle_model: None,
        specific_driver_id: None,
        specific_vehicle_id: None,
        active: true,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

/// `HRMS-603`: same shape `only_an_active_driver_of_the_vehicles_own_tenant_
/// can_be_assigned` proves for `EPIC-FO-03` -- a tenant user, a driver of
/// another tenant and a disabled driver are all refused, and nothing is
/// written.
#[tokio::test]
async fn only_an_active_driver_of_the_demands_own_tenant_may_be_named() {
    let mut disabled = user_row(5, "d@example.com", "Driver", Some(42), "x");
    disabled.enabled = false;
    for candidate in [
        user_row(5, "u@example.com", "TenantUser", Some(42), "x"),
        user_row(5, "d@example.com", "Driver", Some(43), "x"),
        disabled,
    ] {
        let db = mock().append_query_results([[candidate]]).into_connection();
        let mut demand = new_demand(Some(42));
        demand.specific_driver_id = Some(5);
        let refused = run_with_user(Some(owner(42)), demand_use_case(&db).create(demand))
            .await
            .expect_err("not an assignable driver");
        assert_eq!(refused.message, DEMAND_NOT_A_DRIVER);
        assert!(!format!("{:?}", log(db)).contains("INSERT"));
    }
}

/// An unbound platform administrator's read is unrestricted -- `tenant_select`
/// adds no filter for them (`DEF-FO-01`'s lesson) -- so a vehicle from
/// another tenant is returned by the lookup and must be refused by an
/// explicit comparison, not by scope alone.
#[tokio::test]
async fn a_vehicle_outside_the_demands_own_tenant_may_not_be_named() {
    let unbound_admin = AuditUser {
        id: 1,
        email: "admin@example.com".into(),
        tenant_id: None,
        enforce_tenant: false,
    };
    let foreign_vehicle = vehicle_row(10, 99, "ABC1D23", "Active");
    let db = mock()
        .append_query_results([[foreign_vehicle]])
        .into_connection();
    let mut demand = new_demand(Some(42));
    demand.specific_vehicle_id = Some(10);
    let refused = run_with_user(Some(unbound_admin), demand_use_case(&db).create(demand))
        .await
        .expect_err("vehicle belongs to another tenant");
    assert_eq!(refused.message, NOT_A_TENANT_VEHICLE);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

// -------------------------------------------------- EPIC-SC-02-S02 allocation
use business::domain::transport_demand_allocation::TransportDemandAllocation;
use business::gateway::transport_demand_allocation_gateway::TransportDemandAllocationGateway;
use business::use_cases::transport_demand_allocation_use_case::{
    NOT_A_DRIVER as ALLOCATION_NOT_A_DRIVER, NOT_A_TENANT_VEHICLE as ALLOCATION_NOT_A_TENANT_VEHICLE,
    TransportDemandAllocationUseCase,
};

fn allocation_use_case(db: &DatabaseConnection) -> TransportDemandAllocationUseCase {
    TransportDemandAllocationUseCase::new(
        TransportDemandAllocationGateway::new(db.clone()),
        UserGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
    )
}

fn new_allocation(tenant_id: Option<i64>) -> TransportDemandAllocation {
    TransportDemandAllocation {
        id: None,
        uuid: None,
        tenant_id,
        demand_id: 50,
        driver_id: 5,
        vehicle_id: 10,
        days_of_week: None,
        start_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
        end_date: None,
        active: true,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

/// `HRMS-604`: same shape `only_an_active_driver_of_the_demands_own_tenant_
/// may_be_named` proves for `EPIC-SC-02-S01` -- both required here, so the
/// check runs unconditionally rather than only when a value is present.
#[tokio::test]
async fn only_an_active_driver_of_the_allocations_own_tenant_may_be_named() {
    let mut disabled = user_row(5, "d@example.com", "Driver", Some(42), "x");
    disabled.enabled = false;
    for candidate in [
        user_row(5, "u@example.com", "TenantUser", Some(42), "x"),
        user_row(5, "d@example.com", "Driver", Some(43), "x"),
        disabled,
    ] {
        let db = mock().append_query_results([[candidate]]).into_connection();
        let allocation = new_allocation(Some(42));
        let refused = run_with_user(Some(owner(42)), allocation_use_case(&db).create(allocation))
            .await
            .expect_err("not an assignable driver");
        assert_eq!(refused.message, ALLOCATION_NOT_A_DRIVER);
        assert!(!format!("{:?}", log(db)).contains("INSERT"));
    }
}

/// Same `DEF-FO-01` lesson as `a_vehicle_outside_the_demands_own_tenant_
/// may_not_be_named`: an unbound administrator's lookup returns the row
/// regardless of tenant, so the tenant match is checked explicitly.
#[tokio::test]
async fn a_vehicle_outside_the_allocations_own_tenant_may_not_be_named() {
    let unbound_admin = AuditUser {
        id: 1,
        email: "admin@example.com".into(),
        tenant_id: None,
        enforce_tenant: false,
    };
    let db = mock()
        .append_query_results([[user_row(5, "d@example.com", "Driver", Some(42), "x")]])
        .append_query_results([[vehicle_row(10, 99, "ABC1D23", "Active")]])
        .into_connection();
    let allocation = new_allocation(Some(42));
    let refused = run_with_user(Some(unbound_admin), allocation_use_case(&db).create(allocation))
        .await
        .expect_err("vehicle belongs to another tenant");
    assert_eq!(refused.message, ALLOCATION_NOT_A_TENANT_VEHICLE);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

// -------------------------------------------------- EPIC-SC-02-S03 daily schedule
use business::domain::daily_schedule::DailySchedule;
use business::gateway::daily_schedule_gateway::DailyScheduleGateway;
use business::use_cases::daily_schedule_use_case::{
    DailyScheduleUseCase, NOT_A_DRIVER as ENTRY_NOT_A_DRIVER,
    NOT_A_TENANT_VEHICLE as ENTRY_NOT_A_TENANT_VEHICLE,
};

fn schedule_use_case(db: &DatabaseConnection) -> DailyScheduleUseCase {
    DailyScheduleUseCase::new(
        DailyScheduleGateway::new(db.clone()),
        UserGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
    )
}

fn new_schedule_entry(tenant_id: Option<i64>) -> DailySchedule {
    DailySchedule {
        id: None,
        uuid: None,
        tenant_id,
        demand_id: 50,
        driver_id: 5,
        vehicle_id: 10,
        date: NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(),
        start_time: None,
        end_time: None,
        notes: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

/// `HRMS-605`: same shape `only_an_active_driver_of_the_allocations_own_
/// tenant_may_be_named` proves for `EPIC-SC-02-S02`.
#[tokio::test]
async fn only_an_active_driver_of_the_schedule_entrys_own_tenant_may_be_named() {
    let mut disabled = user_row(5, "d@example.com", "Driver", Some(42), "x");
    disabled.enabled = false;
    for candidate in [
        user_row(5, "u@example.com", "TenantUser", Some(42), "x"),
        user_row(5, "d@example.com", "Driver", Some(43), "x"),
        disabled,
    ] {
        let db = mock().append_query_results([[candidate]]).into_connection();
        let entry = new_schedule_entry(Some(42));
        let refused = run_with_user(Some(owner(42)), schedule_use_case(&db).create(entry))
            .await
            .expect_err("not an assignable driver");
        assert_eq!(refused.message, ENTRY_NOT_A_DRIVER);
        assert!(!format!("{:?}", log(db)).contains("INSERT"));
    }
}

/// Same `DEF-FO-01` lesson every other allocation-style use case here
/// proves.
#[tokio::test]
async fn a_vehicle_outside_the_schedule_entrys_own_tenant_may_not_be_named() {
    let unbound_admin = AuditUser {
        id: 1,
        email: "admin@example.com".into(),
        tenant_id: None,
        enforce_tenant: false,
    };
    let db = mock()
        .append_query_results([[user_row(5, "d@example.com", "Driver", Some(42), "x")]])
        .append_query_results([[vehicle_row(10, 99, "ABC1D23", "Active")]])
        .into_connection();
    let entry = new_schedule_entry(Some(42));
    let refused = run_with_user(Some(unbound_admin), schedule_use_case(&db).create(entry))
        .await
        .expect_err("vehicle belongs to another tenant");
    assert_eq!(refused.message, ENTRY_NOT_A_TENANT_VEHICLE);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

// -------------------------------------------------- EPIC-SC-02-S04 schedule exception
use business::domain::schedule_exception::ScheduleException;
use business::gateway::schedule_exception_gateway::ScheduleExceptionGateway;
use business::use_cases::schedule_exception_use_case::{
    NOT_A_DRIVER as EXCEPTION_NOT_A_DRIVER, NOT_A_TENANT_VEHICLE as EXCEPTION_NOT_A_TENANT_VEHICLE,
    ScheduleExceptionUseCase,
};

fn exception_use_case(db: &DatabaseConnection) -> ScheduleExceptionUseCase {
    ScheduleExceptionUseCase::new(
        ScheduleExceptionGateway::new(db.clone()),
        UserGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
    )
}

fn new_exception(tenant_id: Option<i64>) -> ScheduleException {
    ScheduleException {
        id: None,
        uuid: None,
        tenant_id,
        demand_id: 50,
        date: NaiveDate::from_ymd_opt(2026, 1, 6).unwrap(),
        exception_type: business::domain::enums::ScheduleExceptionType::Cancellation,
        new_driver_id: None,
        new_vehicle_id: None,
        reason: None,
        extra_trip_id: None,
        status: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

/// A plain cancellation names neither a driver nor a vehicle, so neither
/// gateway is ever queried -- both checks in `validated` are conditional.
#[tokio::test]
async fn a_cancellation_naming_neither_a_driver_nor_a_vehicle_queries_neither_gateway() {
    let db = mock()
        .append_exec_results([inserted(80)])
        .append_query_results([[schedule_exception_row(80, 42, 50)]])
        .into_connection();
    let exception = new_exception(Some(42));
    let created = run_with_user(Some(owner(42)), exception_use_case(&db).create(exception))
        .await
        .expect("a plain cancellation needs no replacement crew");
    assert_eq!(
        created.exception_type,
        business::domain::enums::ScheduleExceptionType::Cancellation
    );
}

fn schedule_exception_row(id: i64, tenant_id: i64, demand_id: i64) -> schedule_exception_entity::Model {
    schedule_exception_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        demand_id,
        date: NaiveDate::from_ymd_opt(2026, 1, 6).unwrap(),
        exception_type: "Cancellation".into(),
        new_driver_id: None,
        new_vehicle_id: None,
        reason: None,
        extra_trip_id: None,
        status: None,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-606`: when a substitution names a replacement driver, the same
/// active-driver-of-this-tenant check every other use case here runs.
#[tokio::test]
async fn a_named_replacement_driver_must_be_an_active_driver_of_the_exceptions_own_tenant() {
    let mut disabled = user_row(5, "d@example.com", "Driver", Some(42), "x");
    disabled.enabled = false;
    for candidate in [
        user_row(5, "u@example.com", "TenantUser", Some(42), "x"),
        user_row(5, "d@example.com", "Driver", Some(43), "x"),
        disabled,
    ] {
        let db = mock().append_query_results([[candidate]]).into_connection();
        let mut exception = new_exception(Some(42));
        exception.new_driver_id = Some(5);
        let refused = run_with_user(Some(owner(42)), exception_use_case(&db).create(exception))
            .await
            .expect_err("not an assignable driver");
        assert_eq!(refused.message, EXCEPTION_NOT_A_DRIVER);
        assert!(!format!("{:?}", log(db)).contains("INSERT"));
    }
}

/// Same `DEF-FO-01` lesson every other allocation-style use case here
/// proves, for the replacement vehicle.
#[tokio::test]
async fn a_named_replacement_vehicle_must_belong_to_the_exceptions_own_tenant() {
    let unbound_admin = AuditUser {
        id: 1,
        email: "admin@example.com".into(),
        tenant_id: None,
        enforce_tenant: false,
    };
    let db = mock()
        .append_query_results([[vehicle_row(10, 99, "ABC1D23", "Active")]])
        .into_connection();
    let mut exception = new_exception(Some(42));
    exception.new_vehicle_id = Some(10);
    let refused = run_with_user(Some(unbound_admin), exception_use_case(&db).create(exception))
        .await
        .expect_err("vehicle belongs to another tenant");
    assert_eq!(refused.message, EXCEPTION_NOT_A_TENANT_VEHICLE);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

// ---------------------------------------------------------- EPIC-SC-03-S01 extra trip
use business::domain::extra_trip::ExtraTrip;
use business::gateway::customer_gateway::CustomerGateway;
use business::gateway::extra_trip_gateway::ExtraTripGateway;
use business::use_cases::extra_trip_use_case::{
    DUPLICATE_TRIP, ExtraTripUseCase, NOT_A_DRIVER as TRIP_NOT_A_DRIVER,
    NOT_A_TENANT_CUSTOMER, NOT_A_TENANT_VEHICLE as TRIP_NOT_A_TENANT_VEHICLE,
};

fn trip_use_case(db: &DatabaseConnection) -> ExtraTripUseCase {
    ExtraTripUseCase::new(
        ExtraTripGateway::new(db.clone()),
        UserGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        CustomerGateway::new(db.clone()),
    )
}

fn new_trip(tenant_id: Option<i64>) -> ExtraTrip {
    ExtraTrip {
        id: None,
        uuid: None,
        tenant_id,
        order_code: "ORD-100".into(),
        trip_date: NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
        customer_id: None,
        start_time: None,
        return_date: None,
        return_time: None,
        destination: None,
        origin_city: None,
        stops: None,
        preferred_vehicle_type: None,
        driver_id: None,
        second_driver_id: None,
        vehicle_id: None,
        freight_value_cents: None,
        payment: None,
        status: business::domain::enums::TripStatus::Scheduled,
        origin: None,
        import_batch_id: None,
        imported_at: None,
        replaced_by_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

/// `HRMS-607`/`D-24(d)`: same shape `a_plate_already_registered_in_the_
/// same_tenant_is_refused_before_any_insert` proves for `EPIC-FO-01` -- the
/// pre-check reports the collision before any write, tenant-scoped so it
/// can only ever report one inside the caller's own fleet of trips.
#[tokio::test]
async fn a_trip_with_the_same_order_code_and_date_in_the_same_tenant_is_refused_before_any_insert()
{
    let db = mock()
        .append_query_results([[extra_trip_row(1, 42, "ORD-100")]])
        .into_connection();
    let refused = run_with_user(Some(owner(42)), trip_use_case(&db).create(new_trip(Some(42))))
        .await
        .expect_err("a duplicate order code + date is refused");
    assert_eq!(refused.message, DUPLICATE_TRIP);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

fn extra_trip_row(id: i64, tenant_id: i64, order_code: &str) -> entity::extra_trip_entity::Model {
    entity::extra_trip_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        order_code: order_code.into(),
        trip_date: NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
        customer_id: None,
        start_time: None,
        return_date: None,
        return_time: None,
        destination: None,
        origin_city: None,
        stops: None,
        preferred_vehicle_type: None,
        driver_id: None,
        second_driver_id: None,
        vehicle_id: None,
        freight_value_cents: None,
        payment: None,
        status: "Scheduled".into(),
        origin: None,
        import_batch_id: None,
        imported_at: None,
        replaced_by_id: None,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-607`: either driver seat must be an active `Driver` of the trip's
/// own tenant when named.
#[tokio::test]
async fn a_named_driver_in_either_seat_must_be_an_active_driver_of_the_trips_own_tenant() {
    let mut trip = new_trip(Some(42));
    trip.driver_id = Some(5);
    let db = mock()
        .append_query_results([[user_row(5, "u@example.com", "TenantUser", Some(42), "x")]])
        .into_connection();
    let refused = run_with_user(Some(owner(42)), trip_use_case(&db).create(trip))
        .await
        .expect_err("not an assignable driver");
    assert_eq!(refused.message, TRIP_NOT_A_DRIVER);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

/// Same `DEF-FO-01` lesson every other allocation-style use case here
/// proves, for the trip's vehicle.
#[tokio::test]
async fn a_named_vehicle_must_belong_to_the_trips_own_tenant() {
    let unbound_admin = AuditUser {
        id: 1,
        email: "admin@example.com".into(),
        tenant_id: None,
        enforce_tenant: false,
    };
    let mut trip = new_trip(Some(42));
    trip.vehicle_id = Some(10);
    let db = mock()
        .append_query_results([[vehicle_row(10, 99, "ABC1D23", "Active")]])
        .into_connection();
    let refused = run_with_user(Some(unbound_admin), trip_use_case(&db).create(trip))
        .await
        .expect_err("vehicle belongs to another tenant");
    assert_eq!(refused.message, TRIP_NOT_A_TENANT_VEHICLE);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

/// Same `DEF-FO-01` lesson, for the trip's customer.
#[tokio::test]
async fn a_named_customer_must_belong_to_the_trips_own_tenant() {
    let unbound_admin = AuditUser {
        id: 1,
        email: "admin@example.com".into(),
        tenant_id: None,
        enforce_tenant: false,
    };
    let mut trip = new_trip(Some(42));
    trip.customer_id = Some(20);
    let db = mock()
        .append_query_results([[customer_row(20, 99)]])
        .into_connection();
    let refused = run_with_user(Some(unbound_admin), trip_use_case(&db).create(trip))
        .await
        .expect_err("customer belongs to another tenant");
    assert_eq!(refused.message, NOT_A_TENANT_CUSTOMER);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

/// `HRMS-608`: an empty order code is refused before any query -- the same
/// fail-fast shape `a_vehicle_without_a_plate_or_a_model_never_reaches_the_
/// database` proves for `EPIC-FO-01`.
#[tokio::test]
async fn an_empty_order_code_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let mut trip = new_trip(Some(42));
    trip.order_code = "   ".into();
    let result = run_with_user(Some(owner(42)), trip_use_case(&db).import(vec![trip])).await;
    let err = result.expect_err("blank order code is refused");
    assert!(err.english().contains("row 1: order code is required"), "{}", err.english());
    assert!(log(db).is_empty());
}

/// `HRMS-608`/`D-24(d)`: two rows naming the same order code and date are
/// both valid on their own, but not together -- the in-file check
/// `find_duplicate_keys` already proves for city/province, applied to this
/// domain's own identity pair.
#[tokio::test]
async fn two_rows_with_the_same_order_code_and_date_in_one_file_are_rejected_as_duplicates() {
    let db = mock().into_connection();
    let trip = new_trip(Some(42));
    let result = run_with_user(
        Some(owner(42)),
        trip_use_case(&db).import(vec![trip.clone(), trip]),
    )
    .await;
    let err = result.expect_err("duplicate order code + date within the file");
    assert!(
        err.english().contains("row 2: duplicate order code and date within the file"),
        "{}",
        err.english()
    );
    assert!(log(db).is_empty());
}

/// `HRMS-608`/`D-24(d)`: same shape `province_import_creates_new_rows_and_
/// updates_existing_ones` proves -- one row is genuinely new, one already
/// exists by order code + date and is updated in place, not duplicated.
#[tokio::test]
async fn import_creates_new_trips_and_updates_existing_ones_by_code_and_date() {
    let mut new_row = new_trip(Some(42));
    new_row.order_code = "ORD-NEW".into();
    let mut existing_row = new_trip(Some(42));
    existing_row.order_code = "ORD-100".into();

    let db = mock()
        .append_query_results([Vec::<entity::extra_trip_entity::Model>::new()]) // ORD-NEW: not found
        .append_exec_results([inserted(91)])
        .append_query_results([[extra_trip_row(91, 42, "ORD-NEW")]])
        .append_query_results([[extra_trip_row(1, 42, "ORD-100")]]) // ORD-100: exists
        .append_exec_results([inserted(1)])
        .append_query_results([[extra_trip_row(1, 42, "ORD-100")]])
        .into_connection();

    let outcome = run_with_user(
        Some(owner(42)),
        trip_use_case(&db).import(vec![new_row, existing_row]),
    )
    .await
    .expect("both rows are individually valid and not duplicates of each other");
    assert_eq!(outcome.created, 1);
    assert_eq!(outcome.updated, 1);
}

fn customer_row(id: i64, tenant_id: i64) -> entity::customer_entity::Model {
    entity::customer_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        name: "Acme Logistics".into(),
        status: "Active".into(),
        notes: None,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

// -------------------------------------------------------- EPIC-CK-01-S01 km evolution
use business::domain::enums::KmOrigin;
use business::domain::km_evolution::KmEvolution;
use business::gateway::km_evolution_gateway::KmEvolutionGateway;
use business::use_cases::km_evolution_use_case::KmEvolutionUseCase;

fn km_use_case(db: &DatabaseConnection) -> KmEvolutionUseCase {
    KmEvolutionUseCase::new(
        KmEvolutionGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
    )
}

fn new_km_reading(tenant_id: Option<i64>, vehicle_id: i64, km: f64) -> KmEvolution {
    KmEvolution {
        id: None,
        uuid: None,
        tenant_id,
        vehicle_id,
        km,
        recorded_at: None,
        origin: KmOrigin::Adjustment,
        source_entity: None,
        source_entity_id: None,
        notes: None,
        recorded_by_user_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn km_evolution_row(
    id: i64,
    tenant_id: i64,
    vehicle_id: i64,
    km: f64,
    origin: &str,
) -> entity::km_evolution_entity::Model {
    entity::km_evolution_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        vehicle_id,
        km,
        recorded_at: at(),
        origin: origin.into(),
        source_entity: None,
        source_entity_id: None,
        notes: None,
        recorded_by_user_id: None,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-650`: same fail-fast shape `an_invalid_province_never_reaches_the_
/// database` proves -- a negative reading is nonsensical regardless of tenant
/// or vehicle, so it is rejected before any query.
#[tokio::test]
async fn a_negative_odometer_reading_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let reading = new_km_reading(Some(42), 10, -1.0);
    let err = run_with_user(Some(owner(42)), km_use_case(&db).create(reading))
        .await
        .expect_err("a negative reading is refused");
    assert!(err.message.contains("cannot be negative"), "{}", err.message);
    assert!(log(db).is_empty());
}

/// `AD-041`/`TRM-155`: recording a reading recomputes `vehicle.odometer_km`
/// from `find_latest_by_vehicle`'s own query, not from the row just
/// inserted -- proven here by making the just-inserted row an older,
/// backdated `Adjustment` while a different, already-on-file row is the one
/// `find_latest_by_vehicle` reports back as latest.
#[tokio::test]
async fn creating_a_reading_recomputes_the_vehicles_odometer_from_the_query_confirmed_latest_row()
{
    let reading = new_km_reading(Some(42), 10, 500.0);
    let db = mock()
        .append_exec_results([inserted(91)]) // insert the new (backdated) reading
        .append_query_results([[km_evolution_row(91, 42, 10, 500.0, "Adjustment")]]) // refetch after insert
        .append_query_results([[km_evolution_row(50, 42, 10, 12_345.6, "Manual")]]) // find_latest_by_vehicle: a later reading already on file
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]]) // vehicles.find_by_id
        .append_exec_results([inserted(10)]) // update the vehicle
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]]) // refetch after update
        .into_connection();

    let saved = run_with_user(Some(owner(42)), km_use_case(&db).create(reading))
        .await
        .expect("a valid reading is accepted");
    assert_eq!(saved.km, 500.0);

    let sql = format!("{:?}", log(db));
    assert!(
        sql.contains("12345.6"),
        "the vehicle must be stamped with the query-confirmed latest row, not the one just inserted: {sql}"
    );
}

// ---------------------------------------------------- EPIC-CK-02-S01 checklist template
use business::domain::checklist_template::ChecklistTemplate;
use business::domain::checklist_template_item::ChecklistTemplateItem;
use business::domain::enums::ChecklistType;
use business::gateway::checklist_template_gateway::ChecklistTemplateGateway;
use business::gateway::checklist_template_item_gateway::ChecklistTemplateItemGateway;
use business::use_cases::checklist_template_use_case::{
    AT_LEAST_ONE_ITEM_REQUIRED, ChecklistTemplateUseCase, ITEM_DESCRIPTION_REQUIRED, NAME_REQUIRED,
};

fn checklist_template_use_case(db: &DatabaseConnection) -> ChecklistTemplateUseCase {
    ChecklistTemplateUseCase::new(
        ChecklistTemplateGateway::new(db.clone()),
        ChecklistTemplateItemGateway::new(db.clone()),
    )
}

fn new_checklist_template(tenant_id: Option<i64>, name: &str) -> ChecklistTemplate {
    ChecklistTemplate {
        id: None,
        uuid: None,
        tenant_id,
        name: name.into(),
        checklist_type: ChecklistType::Departure,
        active: true,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn new_checklist_template_item(description: &str) -> ChecklistTemplateItem {
    ChecklistTemplateItem {
        id: None,
        uuid: None,
        tenant_id: None,
        checklist_template_id: 0,
        description: description.into(),
        generates_work_order: false,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn checklist_template_row(id: i64, tenant_id: i64, name: &str) -> entity::checklist_template_entity::Model {
    entity::checklist_template_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        name: name.into(),
        checklist_type: "Departure".into(),
        active: true,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

fn checklist_template_item_row(
    id: i64,
    tenant_id: i64,
    template_id: i64,
    description: &str,
) -> entity::checklist_template_item_entity::Model {
    entity::checklist_template_item_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        checklist_template_id: template_id,
        description: description.into(),
        generates_work_order: false,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-651`: same fail-fast shape `an_invalid_province_never_reaches_the_
/// database` proves -- a blank name is refused before the transaction opens.
#[tokio::test]
async fn a_blank_checklist_template_name_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let template = new_checklist_template(Some(42), "   ");
    let items = vec![new_checklist_template_item("Check tyres")];
    let err = run_with_user(
        Some(owner(42)),
        checklist_template_use_case(&db).create(template, items),
    )
    .await
    .expect_err("a blank name is refused");
    assert_eq!(err.message, NAME_REQUIRED);
    assert!(log(db).is_empty());
}

/// `HRMS-651`/`TRM-101`'s own reasoning (a checklist may not carry no
/// items) applied to the template that would produce one.
#[tokio::test]
async fn a_checklist_template_with_no_items_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let template = new_checklist_template(Some(42), "Departure inspection");
    let err = run_with_user(
        Some(owner(42)),
        checklist_template_use_case(&db).create(template, Vec::new()),
    )
    .await
    .expect_err("no items is refused");
    assert_eq!(err.message, AT_LEAST_ONE_ITEM_REQUIRED);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn an_item_with_a_blank_description_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let template = new_checklist_template(Some(42), "Departure inspection");
    let items = vec![new_checklist_template_item("  ")];
    let err = run_with_user(
        Some(owner(42)),
        checklist_template_use_case(&db).create(template, items),
    )
    .await
    .expect_err("a blank item description is refused");
    assert_eq!(err.message, ITEM_DESCRIPTION_REQUIRED);
    assert!(log(db).is_empty());
}

/// `HRMS-651`: the template and its item are written in one transaction --
/// same `PD-027` all-or-nothing reasoning `ExtraTripUseCase::import` uses,
/// applied here to one create with a child row.
#[tokio::test]
async fn creating_a_template_writes_it_and_its_items_in_one_transaction() {
    let template = new_checklist_template(Some(42), "Departure inspection");
    let items = vec![new_checklist_template_item("Check tyre pressure")];

    let db = mock()
        .append_exec_results([inserted(92)]) // insert the template
        .append_query_results([[checklist_template_row(92, 42, "Departure inspection")]]) // refetch after insert
        .append_exec_results([inserted(93)]) // insert the item
        .append_query_results([[checklist_template_item_row(93, 42, 92, "Check tyre pressure")]]) // refetch after insert
        .into_connection();

    let (saved_template, saved_items) = run_with_user(
        Some(owner(42)),
        checklist_template_use_case(&db).create(template, items),
    )
    .await
    .expect("a valid template with items is accepted");

    assert_eq!(saved_template.id, Some(92));
    assert_eq!(saved_items.len(), 1);
    assert_eq!(saved_items[0].checklist_template_id, 92);
}

// ------------------------------------------------------- EPIC-CK-03-S01 checklist run
use business::domain::checklist_answer::ChecklistAnswer;
use business::domain::checklist_run::ChecklistRun;
use business::gateway::checklist_answer_gateway::ChecklistAnswerGateway;
use business::gateway::checklist_run_gateway::ChecklistRunGateway;
use business::use_cases::checklist_run_use_case::{
    AT_LEAST_ONE_ANSWER_REQUIRED, ANSWER_NOT_A_TEMPLATE_ITEM, ChecklistRunUseCase,
    DRIVER_ALREADY_HOLDS_A_VEHICLE, DUPLICATE_ANSWER_FOR_ITEM, MAY_NOT_NAME_AN_OPENING_CHECKLIST,
    NOT_A_DRIVER as RUN_NOT_A_DRIVER, NOT_A_TENANT_TEMPLATE, NOT_A_TENANT_VEHICLE as RUN_NOT_A_TENANT_VEHICLE,
    OPENING_CHECKLIST_ALREADY_CLOSED, OPENING_CHECKLIST_WRONG_VEHICLE, RETURN_REQUIRES_OPENING_CHECKLIST,
};

fn checklist_run_use_case(db: &DatabaseConnection) -> ChecklistRunUseCase {
    ChecklistRunUseCase::new(
        ChecklistRunGateway::new(db.clone()),
        ChecklistTemplateGateway::new(db.clone()),
        ChecklistTemplateItemGateway::new(db.clone()),
        ChecklistAnswerGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        UserGateway::new(db.clone()),
        business::use_cases::km_evolution_use_case::KmEvolutionUseCase::new(
            business::gateway::km_evolution_gateway::KmEvolutionGateway::new(db.clone()),
            VehicleGateway::new(db.clone()),
        ),
        WorkOrderGateway::new(db.clone()),
        WorkOrderItemGateway::new(db.clone()),
    )
}

fn new_checklist_answer(checklist_template_item_id: i64) -> ChecklistAnswer {
    ChecklistAnswer {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        checklist_run_id: 0,
        checklist_template_item_id,
        status: business::domain::enums::AnswerStatus::Conforming,
        observation: None,
        work_order_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn checklist_answer_row(
    id: i64,
    tenant_id: i64,
    checklist_run_id: i64,
    checklist_template_item_id: i64,
) -> entity::checklist_answer_entity::Model {
    entity::checklist_answer_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        checklist_run_id,
        checklist_template_item_id,
        status: "Conforming".into(),
        observation: None,
        work_order_id: None,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

fn new_checklist_run(tenant_id: Option<i64>, checklist_type: ChecklistType) -> ChecklistRun {
    ChecklistRun {
        id: None,
        uuid: None,
        tenant_id,
        checklist_template_id: 92,
        driver_id: 5,
        vehicle_id: 10,
        checklist_type,
        odometer_km: 12_345.6,
        notes: None,
        opening_checklist_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn checklist_run_row(
    id: i64,
    tenant_id: i64,
    checklist_type: &str,
    vehicle_id: i64,
    opening_checklist_id: Option<i64>,
) -> entity::checklist_run_entity::Model {
    entity::checklist_run_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        checklist_template_id: 92,
        driver_id: 5,
        vehicle_id,
        checklist_type: checklist_type.into(),
        odometer_km: 12_345.6,
        notes: None,
        opening_checklist_id,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-652`: same `DEF-FO-01` lesson every other allocation-style use
/// case here proves, for the checklist's template.
#[tokio::test]
async fn a_checklist_template_outside_the_runs_own_tenant_may_not_be_named() {
    let unbound_admin = AuditUser {
        id: 1,
        email: "admin@example.com".into(),
        tenant_id: None,
        enforce_tenant: false,
    };
    let run = new_checklist_run(Some(42), ChecklistType::Standalone);
    let db = mock()
        .append_query_results([[checklist_template_row(92, 99, "Departure inspection")]])
        .into_connection();
    let refused = run_with_user(Some(unbound_admin), checklist_run_use_case(&db).create(run, Vec::new()))
        .await
        .expect_err("template belongs to another tenant");
    assert_eq!(refused.message, NOT_A_TENANT_TEMPLATE);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

#[tokio::test]
async fn a_vehicle_outside_the_runs_own_tenant_may_not_be_named() {
    let unbound_admin = AuditUser {
        id: 1,
        email: "admin@example.com".into(),
        tenant_id: None,
        enforce_tenant: false,
    };
    let run = new_checklist_run(Some(42), ChecklistType::Standalone);
    let db = mock()
        .append_query_results([[checklist_template_row(92, 42, "Departure inspection")]])
        .append_query_results([[vehicle_row(10, 99, "ABC1D23", "Active")]])
        .into_connection();
    let refused = run_with_user(Some(unbound_admin), checklist_run_use_case(&db).create(run, Vec::new()))
        .await
        .expect_err("vehicle belongs to another tenant");
    assert_eq!(refused.message, RUN_NOT_A_TENANT_VEHICLE);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

#[tokio::test]
async fn a_named_driver_must_be_an_active_driver_of_the_runs_own_tenant() {
    let run = new_checklist_run(Some(42), ChecklistType::Standalone);
    let db = mock()
        .append_query_results([[checklist_template_row(92, 42, "Departure inspection")]])
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([[user_row(5, "u@example.com", "TenantUser", Some(42), "x")]])
        .into_connection();
    let refused = run_with_user(Some(owner(42)), checklist_run_use_case(&db).create(run, Vec::new()))
        .await
        .expect_err("not an assignable driver");
    assert_eq!(refused.message, RUN_NOT_A_DRIVER);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

#[tokio::test]
async fn a_departure_may_not_name_an_opening_checklist() {
    let mut run = new_checklist_run(Some(42), ChecklistType::Departure);
    run.opening_checklist_id = Some(1);
    let db = mock()
        .append_query_results([[checklist_template_row(92, 42, "Departure inspection")]])
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([[user_row(5, "d@example.com", "Driver", Some(42), "x")]])
        .into_connection();
    let refused = run_with_user(Some(owner(42)), checklist_run_use_case(&db).create(run, Vec::new()))
        .await
        .expect_err("a departure may not name an opening checklist");
    assert_eq!(refused.message, MAY_NOT_NAME_AN_OPENING_CHECKLIST);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

#[tokio::test]
async fn a_return_must_name_the_departure_it_closes() {
    let run = new_checklist_run(Some(42), ChecklistType::Return);
    let db = mock()
        .append_query_results([[checklist_template_row(92, 42, "Departure inspection")]])
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([[user_row(5, "d@example.com", "Driver", Some(42), "x")]])
        .into_connection();
    let refused = run_with_user(Some(owner(42)), checklist_run_use_case(&db).create(run, Vec::new()))
        .await
        .expect_err("a return with no opening checklist is refused");
    assert_eq!(refused.message, RETURN_REQUIRES_OPENING_CHECKLIST);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

/// `TRM-104`: a return closing a departure of a *different* vehicle makes
/// no sense -- the possession cycle is per vehicle.
#[tokio::test]
async fn a_return_naming_a_departure_of_a_different_vehicle_is_refused() {
    let mut run = new_checklist_run(Some(42), ChecklistType::Return);
    run.opening_checklist_id = Some(1);
    let db = mock()
        .append_query_results([[checklist_template_row(92, 42, "Departure inspection")]])
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([[user_row(5, "d@example.com", "Driver", Some(42), "x")]])
        .append_query_results([[checklist_run_row(1, 42, "Departure", 999, None)]])
        .into_connection();
    let refused = run_with_user(Some(owner(42)), checklist_run_use_case(&db).create(run, Vec::new()))
        .await
        .expect_err("the departure named belongs to a different vehicle");
    assert_eq!(refused.message, OPENING_CHECKLIST_WRONG_VEHICLE);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

/// `TRM-104`: at most one return may close a given departure --
/// `uq_checklist_run_opening_checklist` enforces it at the DB level too.
#[tokio::test]
async fn a_return_naming_an_already_closed_departure_is_refused() {
    let mut run = new_checklist_run(Some(42), ChecklistType::Return);
    run.opening_checklist_id = Some(1);
    let db = mock()
        .append_query_results([[checklist_template_row(92, 42, "Departure inspection")]])
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([[user_row(5, "d@example.com", "Driver", Some(42), "x")]])
        .append_query_results([[checklist_run_row(1, 42, "Departure", 10, None)]])
        .append_query_results([[checklist_run_row(2, 42, "Return", 10, Some(1))]])
        .into_connection();
    let refused = run_with_user(Some(owner(42)), checklist_run_use_case(&db).create(run, Vec::new()))
        .await
        .expect_err("the departure named has already been closed");
    assert_eq!(refused.message, OPENING_CHECKLIST_ALREADY_CLOSED);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

/// `TRM-105`: a driver may hold at most one open vehicle at a time.
#[tokio::test]
async fn a_driver_who_already_holds_an_open_vehicle_may_not_open_a_second_one() {
    let run = new_checklist_run(Some(42), ChecklistType::Departure);
    let db = mock()
        .append_query_results([[checklist_template_row(92, 42, "Departure inspection")]])
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([[user_row(5, "d@example.com", "Driver", Some(42), "x")]])
        .append_query_results([[checklist_run_row(1, 42, "Departure", 99, None)]]) // find_departures_by_driver
        .append_query_results([Vec::<entity::checklist_run_entity::Model>::new()]) // find_returns_closing: none close it
        .into_connection();
    let refused = run_with_user(Some(owner(42)), checklist_run_use_case(&db).create(run, Vec::new()))
        .await
        .expect_err("the driver already holds a vehicle");
    assert_eq!(refused.message, DRIVER_ALREADY_HOLDS_A_VEHICLE);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

/// `TRM-101`: a checklist with no answered items is refused before any
/// write -- the same fail-fast shape every other empty-collection check in
/// this file proves.
#[tokio::test]
async fn a_checklist_with_no_answered_items_is_rejected_before_any_write() {
    let run = new_checklist_run(Some(42), ChecklistType::Standalone);
    let db = mock()
        .append_query_results([[checklist_template_row(92, 42, "Departure inspection")]])
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([[user_row(5, "d@example.com", "Driver", Some(42), "x")]])
        .into_connection();
    let refused = run_with_user(Some(owner(42)), checklist_run_use_case(&db).create(run, Vec::new()))
        .await
        .expect_err("no answered items is refused");
    assert_eq!(refused.message, AT_LEAST_ONE_ANSWER_REQUIRED);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

#[tokio::test]
async fn answering_the_same_item_twice_in_one_submission_is_refused() {
    let run = new_checklist_run(Some(42), ChecklistType::Standalone);
    let answers = vec![new_checklist_answer(95), new_checklist_answer(95)];
    let db = mock()
        .append_query_results([[checklist_template_row(92, 42, "Departure inspection")]])
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([[user_row(5, "d@example.com", "Driver", Some(42), "x")]])
        .append_query_results([[checklist_template_item_row(95, 42, 92, "Check tyres")]]) // first answer's item lookup; the second is the duplicate and never gets one
        .into_connection();
    let refused = run_with_user(Some(owner(42)), checklist_run_use_case(&db).create(run, answers))
        .await
        .expect_err("the same item answered twice is refused");
    assert_eq!(refused.message, DUPLICATE_ANSWER_FOR_ITEM);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

/// `DEF-FO-01`'s lesson applied to the answer's own item: it must belong to
/// the checklist's own template, not some other template's item.
#[tokio::test]
async fn an_answer_naming_an_item_of_a_different_template_is_refused() {
    let run = new_checklist_run(Some(42), ChecklistType::Standalone);
    let answers = vec![new_checklist_answer(95)];
    let db = mock()
        .append_query_results([[checklist_template_row(92, 42, "Departure inspection")]])
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([[user_row(5, "d@example.com", "Driver", Some(42), "x")]])
        .append_query_results([[checklist_template_item_row(95, 42, 999, "Check tyres")]])
        .into_connection();
    let refused = run_with_user(Some(owner(42)), checklist_run_use_case(&db).create(run, answers))
        .await
        .expect_err("the item belongs to a different template");
    assert_eq!(refused.message, ANSWER_NOT_A_TEMPLATE_ITEM);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

/// `HRMS-652`/`HRMS-654`/`AD-041`: a successful `Standalone` run and its
/// answer are written together, and the vehicle's official odometer entry
/// is written through the one writer `EPIC-CK-01-S01` built -- proving the
/// three use cases are actually wired together, not just individually
/// correct.
#[tokio::test]
async fn a_valid_standalone_checklist_and_its_answer_are_recorded_and_write_the_vehicles_official_odometer()
{
    let run = new_checklist_run(Some(42), ChecklistType::Standalone);
    let answers = vec![new_checklist_answer(95)];
    let db = mock()
        .append_query_results([[checklist_template_row(92, 42, "Departure inspection")]]) // template
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]]) // vehicle
        .append_query_results([[user_row(5, "d@example.com", "Driver", Some(42), "x")]]) // driver
        .append_query_results([[checklist_template_item_row(95, 42, 92, "Check tyres")]]) // answer's own item
        .append_exec_results([inserted(94)]) // insert the run
        .append_query_results([[checklist_run_row(94, 42, "Standalone", 10, None)]]) // refetch after insert
        .append_exec_results([inserted(96)]) // insert the answer
        .append_query_results([[checklist_answer_row(96, 42, 94, 95)]]) // refetch after insert
        .append_exec_results([inserted(200)]) // insert the km_evolution reading
        .append_query_results([[km_evolution_row(200, 42, 10, 12_345.6, "DriverChecklist")]]) // refetch after insert
        .append_query_results([[km_evolution_row(200, 42, 10, 12_345.6, "DriverChecklist")]]) // find_latest_by_vehicle
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]]) // vehicles.find_by_id (recompute)
        .append_exec_results([inserted(10)]) // update the vehicle
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]]) // refetch after update
        .into_connection();

    let (saved, saved_answers) = run_with_user(
        Some(owner(42)),
        checklist_run_use_case(&db).create(run, answers),
    )
    .await
    .expect("a valid standalone checklist with one answer is accepted");
    assert_eq!(saved.id, Some(94));
    assert_eq!(saved.checklist_type, ChecklistType::Standalone);
    assert_eq!(saved_answers.len(), 1);
    assert_eq!(saved_answers[0].checklist_run_id, 94);

    let sql = format!("{:?}", log(db));
    assert!(
        sql.contains("12345.6"),
        "the checklist's reading must reach km_evolution too: {sql}"
    );
}

// ------------------------------------------------------- EPIC-CK-03-S02 checklist -> work order
/// `TRM-115`/`TRM-116`: a flagged, non-conforming answer opens exactly one
/// work order, with one work-order item carrying the flagged template
/// item's own description and the driver's observation, and the answer is
/// updated to name the work order it opened.
#[tokio::test]
async fn a_flagged_non_conforming_answer_opens_exactly_one_work_order() {
    let run = new_checklist_run(Some(42), business::domain::enums::ChecklistType::Standalone);
    let mut answer = new_checklist_answer(95);
    answer.status = business::domain::enums::AnswerStatus::NonConforming;
    answer.observation = Some("Pad worn to metal".into());

    let db = mock()
        .append_query_results([[checklist_template_row(92, 42, "Departure inspection")]]) // template
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]]) // vehicle
        .append_query_results([[user_row(5, "d@example.com", "Driver", Some(42), "x")]]) // driver
        .append_query_results([[entity::checklist_template_item_entity::Model {
            generates_work_order: true,
            ..checklist_template_item_row(95, 42, 92, "Check brake pads")
        }]]) // answer's own item, flagged
        .append_exec_results([inserted(94)]) // insert the run
        .append_query_results([[checklist_run_row(94, 42, "Standalone", 10, None)]]) // refetch after insert
        .append_exec_results([inserted(96)]) // insert the answer
        .append_query_results([[entity::checklist_answer_entity::Model {
            status: "NonConforming".into(),
            ..checklist_answer_row(96, 42, 94, 95)
        }]]) // refetch after insert
        .append_exec_results([inserted(200)]) // insert the km_evolution reading
        .append_query_results([[km_evolution_row(200, 42, 10, 12_345.6, "DriverChecklist")]]) // refetch after insert
        .append_query_results([[km_evolution_row(200, 42, 10, 12_345.6, "DriverChecklist")]]) // find_latest_by_vehicle
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]]) // vehicles.find_by_id (recompute)
        .append_exec_results([inserted(10)]) // update the vehicle
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]]) // refetch after update
        .append_exec_results([inserted(300)]) // insert the work order
        .append_query_results([[entity::work_order_entity::Model {
            origin: "Checklist".into(),
            checklist_run_id: Some(94),
            ..work_order_row(300, 42, 10)
        }]]) // refetch after insert
        .append_exec_results([inserted(301)]) // insert the work order item
        .append_query_results([[work_order_item_row(301, 42, 300, "Pending")]]) // refetch after insert
        .append_exec_results([inserted(96)]) // update the answer's work_order_id
        .append_query_results([[entity::checklist_answer_entity::Model {
            status: "NonConforming".into(),
            work_order_id: Some(300),
            ..checklist_answer_row(96, 42, 94, 95)
        }]]) // refetch after update
        .into_connection();

    let (_, saved_answers) = run_with_user(
        Some(owner(42)),
        checklist_run_use_case(&db).create(run, vec![answer]),
    )
    .await
    .expect("a flagged non-conforming answer is accepted");

    assert_eq!(saved_answers.len(), 1);
    assert_eq!(
        saved_answers[0].work_order_id,
        Some(300),
        "the answer must name the work order it opened"
    );

    let sql = format!("{:?}", log(db));
    assert!(
        sql.contains("Check brake pads"),
        "the work-order item must carry the template item's own description: {sql}"
    );
}

/// A flagged item whose answer is `Conforming` opens nothing -- only a
/// non-conforming answer does (`TRM-115`).
#[tokio::test]
async fn a_flagged_conforming_answer_does_not_open_a_work_order() {
    let run = new_checklist_run(Some(42), business::domain::enums::ChecklistType::Standalone);
    let answer = new_checklist_answer(95); // Conforming by default

    let db = mock()
        .append_query_results([[checklist_template_row(92, 42, "Departure inspection")]])
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([[user_row(5, "d@example.com", "Driver", Some(42), "x")]])
        .append_query_results([[entity::checklist_template_item_entity::Model {
            generates_work_order: true,
            ..checklist_template_item_row(95, 42, 92, "Check brake pads")
        }]])
        .append_exec_results([inserted(94)])
        .append_query_results([[checklist_run_row(94, 42, "Standalone", 10, None)]])
        .append_exec_results([inserted(96)])
        .append_query_results([[checklist_answer_row(96, 42, 94, 95)]])
        .append_exec_results([inserted(200)])
        .append_query_results([[km_evolution_row(200, 42, 10, 12_345.6, "DriverChecklist")]])
        .append_query_results([[km_evolution_row(200, 42, 10, 12_345.6, "DriverChecklist")]])
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_exec_results([inserted(10)])
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .into_connection();

    let (_, saved_answers) = run_with_user(
        Some(owner(42)),
        checklist_run_use_case(&db).create(run, vec![answer]),
    )
    .await
    .expect("a conforming answer is accepted");

    assert_eq!(saved_answers[0].work_order_id, None);
    assert!(!format!("{:?}", log(db)).contains("Check brake pads"));
}

/// A non-conforming answer whose template item does not carry the flag
/// opens nothing either -- the flag decides, not the answer alone.
#[tokio::test]
async fn a_non_conforming_answer_on_an_unflagged_item_does_not_open_a_work_order() {
    let run = new_checklist_run(Some(42), business::domain::enums::ChecklistType::Standalone);
    let mut answer = new_checklist_answer(95);
    answer.status = business::domain::enums::AnswerStatus::NonConforming;

    let db = mock()
        .append_query_results([[checklist_template_row(92, 42, "Departure inspection")]])
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([[user_row(5, "d@example.com", "Driver", Some(42), "x")]])
        .append_query_results([[checklist_template_item_row(95, 42, 92, "Check tyres")]]) // not flagged
        .append_exec_results([inserted(94)])
        .append_query_results([[checklist_run_row(94, 42, "Standalone", 10, None)]])
        .append_exec_results([inserted(96)])
        .append_query_results([[entity::checklist_answer_entity::Model {
            status: "NonConforming".into(),
            ..checklist_answer_row(96, 42, 94, 95)
        }]])
        .append_exec_results([inserted(200)])
        .append_query_results([[km_evolution_row(200, 42, 10, 12_345.6, "DriverChecklist")]])
        .append_query_results([[km_evolution_row(200, 42, 10, 12_345.6, "DriverChecklist")]])
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_exec_results([inserted(10)])
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .into_connection();

    let (_, saved_answers) = run_with_user(
        Some(owner(42)),
        checklist_run_use_case(&db).create(run, vec![answer]),
    )
    .await
    .expect("a non-conforming answer on an unflagged item is accepted");

    assert_eq!(saved_answers[0].work_order_id, None);
}

// ------------------------------------------------------- EPIC-MT-01-S01 work order
use business::domain::enums::{WorkOrderItemStatus, WorkOrderOrigin, WorkOrderStatus};
use business::domain::work_order::WorkOrder;
use business::domain::work_order_item::WorkOrderItem;
use business::gateway::work_order_gateway::WorkOrderGateway;
use business::gateway::work_order_item_gateway::WorkOrderItemGateway;
use business::use_cases::work_order_use_case::{
    DESCRIPTION_REQUIRED, ITEM_DESCRIPTION_REQUIRED as WO_ITEM_DESCRIPTION_REQUIRED,
    ITEM_NOT_ON_THIS_WORK_ORDER, NEGATIVE_ODOMETER, NOT_A_TENANT_VEHICLE as WO_NOT_A_TENANT_VEHICLE,
    WORK_ORDER_NOT_FOUND, WorkOrderUseCase,
};

fn work_order_use_case(db: &DatabaseConnection) -> WorkOrderUseCase {
    WorkOrderUseCase::new(
        WorkOrderGateway::new(db.clone()),
        WorkOrderItemGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        business::use_cases::km_evolution_use_case::KmEvolutionUseCase::new(
            business::gateway::km_evolution_gateway::KmEvolutionGateway::new(db.clone()),
            VehicleGateway::new(db.clone()),
        ),
        business::gateway::preventive_plan_gateway::PreventivePlanGateway::new(db.clone()),
        business::gateway::preventive_plan_extension_gateway::PreventivePlanExtensionGateway::new(db.clone()),
    )
}

fn new_work_order(tenant_id: Option<i64>, description: &str, odometer_km: f64) -> WorkOrder {
    WorkOrder {
        id: None,
        uuid: None,
        tenant_id,
        vehicle_id: 10,
        opened_at: None,
        odometer_km,
        origin: WorkOrderOrigin::Manual,
        checklist_run_id: None,
        maintenance_plan_id: None,
        preventive_plan_id: None,
        service_type: None,
        description: description.into(),
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

fn work_order_row(id: i64, tenant_id: i64, vehicle_id: i64) -> entity::work_order_entity::Model {
    entity::work_order_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        vehicle_id,
        opened_at: at(),
        odometer_km: 12_345.6,
        origin: "Manual".into(),
        checklist_run_id: None,
        maintenance_plan_id: None,
        preventive_plan_id: None,
        service_type: None,
        description: "Squeaking front brakes".into(),
        responsible: None,
        status: "Open".into(),
        observation: None,
        external_service: false,
        supplier: None,
        invoice_number: None,
        invoice_value_cents: None,
        invoice_date: None,
        concluded_at: None,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-700`: same fail-fast shape `an_invalid_province_never_reaches_the_
/// database` proves -- a blank description never reaches the database.
#[tokio::test]
async fn a_blank_work_order_description_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let work_order = new_work_order(Some(42), "   ", 12_345.6);
    let err = run_with_user(Some(owner(42)), work_order_use_case(&db).create(work_order))
        .await
        .expect_err("a blank description is refused");
    assert_eq!(err.message, DESCRIPTION_REQUIRED);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn a_negative_work_order_odometer_reading_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let work_order = new_work_order(Some(42), "Squeaking front brakes", -1.0);
    let err = run_with_user(Some(owner(42)), work_order_use_case(&db).create(work_order))
        .await
        .expect_err("a negative reading is refused");
    assert_eq!(err.message, NEGATIVE_ODOMETER);
    assert!(log(db).is_empty());
}

/// Same `DEF-FO-01` lesson every other allocation-style use case here
/// proves, for the work order's vehicle.
#[tokio::test]
async fn a_work_order_vehicle_must_belong_to_the_orders_own_tenant() {
    let unbound_admin = AuditUser {
        id: 1,
        email: "admin@example.com".into(),
        tenant_id: None,
        enforce_tenant: false,
    };
    let work_order = new_work_order(Some(42), "Squeaking front brakes", 12_345.6);
    let db = mock()
        .append_query_results([[vehicle_row(10, 99, "ABC1D23", "Active")]])
        .into_connection();
    let refused = run_with_user(Some(unbound_admin), work_order_use_case(&db).create(work_order))
        .await
        .expect_err("vehicle belongs to another tenant");
    assert_eq!(refused.message, WO_NOT_A_TENANT_VEHICLE);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

/// `HRMS-700`/`AD-041`: a successful work order is opened, and the
/// vehicle's official odometer entry is written through the one writer
/// `EPIC-CK-01-S01` built -- proving the two use cases are actually wired
/// together, not just individually correct.
#[tokio::test]
async fn a_valid_work_order_is_opened_and_writes_the_vehicles_official_odometer() {
    let work_order = new_work_order(Some(42), "Squeaking front brakes", 12_345.6);
    let db = mock()
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]]) // vehicle
        .append_exec_results([inserted(96)]) // insert the work order
        .append_query_results([[work_order_row(96, 42, 10)]]) // refetch after insert
        .append_exec_results([inserted(200)]) // insert the km_evolution reading
        .append_query_results([[km_evolution_row(200, 42, 10, 12_345.6, "WorkOrder")]]) // refetch after insert
        .append_query_results([[km_evolution_row(200, 42, 10, 12_345.6, "WorkOrder")]]) // find_latest_by_vehicle
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]]) // vehicles.find_by_id (recompute)
        .append_exec_results([inserted(10)]) // update the vehicle
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]]) // refetch after update
        .into_connection();

    let saved = run_with_user(Some(owner(42)), work_order_use_case(&db).create(work_order))
        .await
        .expect("a valid work order is accepted");
    assert_eq!(saved.id, Some(96));
    assert_eq!(saved.status, WorkOrderStatus::Open);
    assert_eq!(saved.origin, WorkOrderOrigin::Manual);

    let sql = format!("{:?}", log(db));
    assert!(
        sql.contains("12345.6"),
        "the vehicle's odometer must reach km_evolution too: {sql}"
    );
}

// ------------------------------------------------------- EPIC-MT-01-S02 work order items
fn work_order_row_with_status(
    id: i64,
    tenant_id: i64,
    vehicle_id: i64,
    status: &str,
) -> entity::work_order_entity::Model {
    entity::work_order_entity::Model {
        status: status.into(),
        ..work_order_row(id, tenant_id, vehicle_id)
    }
}

fn new_work_order_item(status: WorkOrderItemStatus) -> WorkOrderItem {
    WorkOrderItem {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        work_order_id: 0,
        description: "Check brake pads".into(),
        item_type: None,
        status,
        observation: None,
        resolved_by: None,
        resolved_at: None,
        resolution_description: None,
        purchase_order_id: None,
        is_purchase_placeholder: false,
        preventive_plan_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn work_order_item_row(
    id: i64,
    tenant_id: i64,
    work_order_id: i64,
    status: &str,
) -> entity::work_order_item_entity::Model {
    entity::work_order_item_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        work_order_id,
        description: "Check brake pads".into(),
        item_type: None,
        status: status.into(),
        observation: None,
        resolved_by: None,
        resolved_at: None,
        resolution_description: None,
        purchase_order_id: None,
        is_purchase_placeholder: false,
        preventive_plan_id: None,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

#[tokio::test]
async fn a_blank_work_order_item_description_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let mut item = new_work_order_item(WorkOrderItemStatus::Pending);
    item.description = "   ".into();
    let err = run_with_user(Some(owner(42)), work_order_use_case(&db).add_item(96, item))
        .await
        .expect_err("a blank item description is refused");
    assert_eq!(err.message, WO_ITEM_DESCRIPTION_REQUIRED);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn adding_an_item_to_a_missing_work_order_is_rejected() {
    let db = mock()
        .append_query_results([Vec::<entity::work_order_entity::Model>::new()])
        .into_connection();
    let item = new_work_order_item(WorkOrderItemStatus::Pending);
    let err = run_with_user(Some(owner(42)), work_order_use_case(&db).add_item(96, item))
        .await
        .expect_err("no such work order");
    assert_eq!(err.message, WORK_ORDER_NOT_FOUND);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

/// `TRM-206`/`TRM-207`: the first item on an `Open` order, still pending,
/// does not by itself make anything "partially" resolved.
#[tokio::test]
async fn adding_a_lone_pending_item_to_an_open_work_order_keeps_it_open() {
    let item = new_work_order_item(WorkOrderItemStatus::Pending);
    let db = mock()
        .append_query_results([[work_order_row_with_status(96, 42, 10, "Open")]]) // find_by_id
        .append_exec_results([inserted(97)]) // insert the item
        .append_query_results([[work_order_item_row(97, 42, 96, "Pending")]]) // refetch after insert
        .append_query_results([[work_order_item_row(97, 42, 96, "Pending")]]) // find_by_work_order
        .into_connection();

    let (work_order, saved_item) = run_with_user(Some(owner(42)), work_order_use_case(&db).add_item(96, item))
        .await
        .expect("a valid item is accepted");
    assert_eq!(work_order.status, WorkOrderStatus::Open);
    assert_eq!(saved_item.id, Some(97));
    assert!(
        !format!("{:?}", log(db)).contains("UPDATE"),
        "no status change means no second write"
    );
}

/// `TRM-207`: work has been done (one item already resolved) and a
/// pendency remains (the new one) -- exactly "partially resolved".
#[tokio::test]
async fn adding_a_pending_item_alongside_a_resolved_one_sets_the_work_order_partially_resolved() {
    let item = new_work_order_item(WorkOrderItemStatus::Pending);
    let db = mock()
        .append_query_results([[work_order_row_with_status(96, 42, 10, "Open")]]) // find_by_id
        .append_exec_results([inserted(98)]) // insert the new item
        .append_query_results([[work_order_item_row(98, 42, 96, "Pending")]]) // refetch after insert
        .append_query_results([[
            work_order_item_row(97, 42, 96, "Resolved"),
            work_order_item_row(98, 42, 96, "Pending"),
        ]]) // find_by_work_order: both items
        .append_exec_results([inserted(96)]) // update the work order's status
        .append_query_results([[work_order_row_with_status(96, 42, 10, "PartiallyResolved")]]) // refetch after update
        .into_connection();

    let (work_order, _) = run_with_user(Some(owner(42)), work_order_use_case(&db).add_item(96, item))
        .await
        .expect("a valid item is accepted");
    assert_eq!(work_order.status, WorkOrderStatus::PartiallyResolved);
}

/// `TRM-206`: any item awaiting a part holds the whole order in
/// `AwaitingParts`, regardless of the other items.
#[tokio::test]
async fn adding_an_awaiting_parts_item_holds_the_work_order_in_awaiting_parts() {
    let item = new_work_order_item(WorkOrderItemStatus::AwaitingParts);
    let db = mock()
        .append_query_results([[work_order_row_with_status(96, 42, 10, "Open")]])
        .append_exec_results([inserted(99)])
        .append_query_results([[work_order_item_row(99, 42, 96, "AwaitingParts")]])
        .append_query_results([[work_order_item_row(99, 42, 96, "AwaitingParts")]])
        .append_exec_results([inserted(96)])
        .append_query_results([[work_order_row_with_status(96, 42, 10, "AwaitingParts")]])
        .into_connection();

    let (work_order, _) = run_with_user(Some(owner(42)), work_order_use_case(&db).add_item(96, item))
        .await
        .expect("a valid item is accepted");
    assert_eq!(work_order.status, WorkOrderStatus::AwaitingParts);
}

/// `TRM-215`: a concluded order is not re-evaluated by an item that is not
/// a genuinely new pending one.
#[tokio::test]
async fn adding_a_resolved_item_to_a_concluded_work_order_leaves_it_untouched() {
    let item = new_work_order_item(WorkOrderItemStatus::Resolved);
    let db = mock()
        .append_query_results([[work_order_row_with_status(96, 42, 10, "Concluded")]])
        .append_exec_results([inserted(100)])
        .append_query_results([[work_order_item_row(100, 42, 96, "Resolved")]])
        .append_query_results([[work_order_item_row(100, 42, 96, "Resolved")]])
        .into_connection();

    let (work_order, _) = run_with_user(Some(owner(42)), work_order_use_case(&db).add_item(96, item))
        .await
        .expect("a valid item is accepted");
    assert_eq!(work_order.status, WorkOrderStatus::Concluded);
    assert!(
        !format!("{:?}", log(db)).contains("UPDATE"),
        "TRM-215: a non-pending item must not touch a concluded order"
    );
}

/// `TRM-215`: "only a genuinely new pending item shall reopen it."
#[tokio::test]
async fn adding_a_pending_item_to_a_concluded_work_order_reopens_it() {
    let item = new_work_order_item(WorkOrderItemStatus::Pending);
    let db = mock()
        .append_query_results([[work_order_row_with_status(96, 42, 10, "Concluded")]])
        .append_exec_results([inserted(101)])
        .append_query_results([[work_order_item_row(101, 42, 96, "Pending")]])
        .append_query_results([[work_order_item_row(101, 42, 96, "Pending")]])
        .append_exec_results([inserted(96)])
        .append_query_results([[work_order_row_with_status(96, 42, 10, "Open")]])
        .into_connection();

    let (work_order, _) = run_with_user(Some(owner(42)), work_order_use_case(&db).add_item(96, item))
        .await
        .expect("a valid item is accepted");
    assert_eq!(
        work_order.status,
        WorkOrderStatus::Open,
        "a genuinely new pending item reopens a concluded order"
    );
}

// ------------------------------------------------------- EPIC-MT-01-S03 conclude/cancel
/// `TRM-202`: a work order with no items at all has nothing left pending,
/// so concluding it succeeds immediately.
#[tokio::test]
async fn concluding_a_work_order_with_no_items_succeeds_immediately() {
    let db = mock()
        .append_query_results([[work_order_row_with_status(96, 42, 10, "Open")]]) // find_by_id
        .append_query_results([Vec::<entity::work_order_item_entity::Model>::new()]) // find_by_work_order: none
        .append_exec_results([inserted(96)]) // update the work order
        .append_query_results([[work_order_row_with_status(96, 42, 10, "Concluded")]]) // refetch after update
        .into_connection();

    let work_order = run_with_user(Some(owner(42)), work_order_use_case(&db).conclude(96))
        .await
        .expect("concluding an itemless order succeeds");
    assert_eq!(work_order.status, WorkOrderStatus::Concluded);
}

#[tokio::test]
async fn concluding_a_work_order_whose_items_are_all_settled_concludes_it() {
    let db = mock()
        .append_query_results([[work_order_row_with_status(96, 42, 10, "PartiallyResolved")]])
        .append_query_results([[
            work_order_item_row(97, 42, 96, "Resolved"),
            work_order_item_row(98, 42, 96, "Cancelled"),
        ]])
        .append_exec_results([inserted(96)])
        .append_query_results([[work_order_row_with_status(96, 42, 10, "Concluded")]])
        .into_connection();

    let work_order = run_with_user(Some(owner(42)), work_order_use_case(&db).conclude(96))
        .await
        .expect("concluding a fully-settled order succeeds");
    assert_eq!(work_order.status, WorkOrderStatus::Concluded);
}

/// `TRM-204`: concluding with a pendency still open does not fail and does
/// not conclude -- it downgrades to `PartiallyResolved`, items untouched.
#[tokio::test]
async fn concluding_a_work_order_with_a_pending_item_downgrades_to_partially_resolved() {
    let db = mock()
        .append_query_results([[work_order_row_with_status(96, 42, 10, "Open")]])
        .append_query_results([[work_order_item_row(97, 42, 96, "Pending")]])
        .append_exec_results([inserted(96)])
        .append_query_results([[work_order_row_with_status(96, 42, 10, "PartiallyResolved")]])
        .into_connection();

    let work_order = run_with_user(Some(owner(42)), work_order_use_case(&db).conclude(96))
        .await
        .expect("conclude never fails, it downgrades instead");
    assert_eq!(work_order.status, WorkOrderStatus::PartiallyResolved);
}

/// `TRM-215`: an already-concluded order is not re-evaluated at all.
#[tokio::test]
async fn concluding_an_already_concluded_work_order_is_a_no_op() {
    let db = mock()
        .append_query_results([[work_order_row_with_status(96, 42, 10, "Concluded")]])
        .into_connection();

    let work_order = run_with_user(Some(owner(42)), work_order_use_case(&db).conclude(96))
        .await
        .expect("a no-op still succeeds");
    assert_eq!(work_order.status, WorkOrderStatus::Concluded);
    assert!(!format!("{:?}", log(db)).contains("UPDATE"), "a terminal order is not re-stamped");
}

/// `TRM-205`: cancelling closes a work order regardless of outstanding
/// items -- no item query is even needed.
#[tokio::test]
async fn cancelling_a_work_order_succeeds_regardless_of_outstanding_items() {
    let db = mock()
        .append_query_results([[work_order_row_with_status(96, 42, 10, "AwaitingParts")]])
        .append_exec_results([inserted(96)])
        .append_query_results([[work_order_row_with_status(96, 42, 10, "Cancelled")]])
        .into_connection();

    let work_order = run_with_user(Some(owner(42)), work_order_use_case(&db).cancel(96))
        .await
        .expect("cancel never checks items");
    assert_eq!(work_order.status, WorkOrderStatus::Cancelled);
}

#[tokio::test]
async fn cancelling_an_already_cancelled_work_order_is_a_no_op() {
    let db = mock()
        .append_query_results([[work_order_row_with_status(96, 42, 10, "Cancelled")]])
        .into_connection();

    let work_order = run_with_user(Some(owner(42)), work_order_use_case(&db).cancel(96))
        .await
        .expect("a no-op still succeeds");
    assert_eq!(work_order.status, WorkOrderStatus::Cancelled);
    assert!(!format!("{:?}", log(db)).contains("UPDATE"), "a terminal order is not re-stamped");
}

// ------------------------------------------------------- EPIC-SP-03-S02 purchase-order pendency wiring
/// `TRM-644`/`646`: naming an existing pendency moves it to `AwaitingParts`
/// and recomputes the parent work order the same way `add_item` does.
#[tokio::test]
async fn linking_an_existing_pendency_to_a_purchase_order_sets_it_awaiting_parts_and_recomputes_the_work_order()
{
    let db = mock()
        .append_query_results([[work_order_row_with_status(96, 42, 10, "Open")]]) // the work order
        .append_query_results([[work_order_item_row(97, 42, 96, "Pending")]]) // the named pendency
        .append_exec_results([inserted(97)]) // item update
        .append_query_results([[entity::work_order_item_entity::Model {
            status: "AwaitingParts".into(),
            purchase_order_id: Some(950),
            ..work_order_item_row(97, 42, 96, "AwaitingParts")
        }]]) // item refetch
        .append_query_results([[entity::work_order_item_entity::Model {
            status: "AwaitingParts".into(),
            purchase_order_id: Some(950),
            ..work_order_item_row(97, 42, 96, "AwaitingParts")
        }]]) // all items on the work order
        .append_exec_results([inserted(96)]) // work order update
        .append_query_results([[work_order_row_with_status(96, 42, 10, "AwaitingParts")]]) // refetch
        .into_connection();

    let updated = run_with_user(
        Some(owner(42)),
        work_order_use_case(&db).mark_item_awaiting_parts(96, 97, 950),
    )
    .await
    .expect("linking an existing pendency succeeds");
    assert_eq!(updated.status, WorkOrderStatus::AwaitingParts);
}

#[tokio::test]
async fn linking_a_pendency_from_a_different_work_order_is_refused() {
    let db = mock()
        .append_query_results([[work_order_row_with_status(96, 42, 10, "Open")]])
        .append_query_results([[work_order_item_row(97, 42, 999, "Pending")]]) // belongs to a different order
        .into_connection();

    let err = run_with_user(
        Some(owner(42)),
        work_order_use_case(&db).mark_item_awaiting_parts(96, 97, 950),
    )
    .await
    .expect_err("an item from another work order is refused");
    assert_eq!(err.message, ITEM_NOT_ON_THIS_WORK_ORDER);
}

/// `TRM-647`: a synthetic placeholder only ever represented the purchase
/// itself, so receiving it resolves the item outright. As the work order's
/// sole item, `recompute_status`'s own "nothing left pending" fallback
/// leaves the order's status untouched (`EPIC-MT-01-S03`'s existing rule:
/// conclusion is a deliberate administrative act, never inferred) -- so
/// `AwaitingParts` persists here until someone calls `conclude`.
#[tokio::test]
async fn resolving_a_placeholder_pendency_marks_it_resolved() {
    let db = mock()
        .append_query_results([[entity::work_order_item_entity::Model {
            is_purchase_placeholder: true,
            preventive_plan_id: None,
            purchase_order_id: Some(950),
            ..work_order_item_row(98, 42, 96, "AwaitingParts")
        }]]) // the placeholder item
        .append_query_results([[work_order_row_with_status(96, 42, 10, "AwaitingParts")]]) // its work order
        .append_exec_results([inserted(98)]) // item update
        .append_query_results([[entity::work_order_item_entity::Model {
            is_purchase_placeholder: true,
            preventive_plan_id: None,
            purchase_order_id: Some(950),
            ..work_order_item_row(98, 42, 96, "Resolved")
        }]]) // item refetch
        .append_query_results([[entity::work_order_item_entity::Model {
            is_purchase_placeholder: true,
            preventive_plan_id: None,
            purchase_order_id: Some(950),
            ..work_order_item_row(98, 42, 96, "Resolved")
        }]]) // all items -- only the placeholder, now resolved
        .into_connection();

    let updated = run_with_user(Some(owner(42)), work_order_use_case(&db).resolve_purchase_pendency(98))
        .await
        .expect("resolving a placeholder pendency succeeds");
    assert_eq!(updated.status, WorkOrderStatus::AwaitingParts);
}

/// `TRM-648`/`649`: a real pendency returns to `Pending` -- receiving the
/// part does not mean the repair was done -- and the work order's status
/// recomputes away from `AwaitingParts` the moment nothing still awaits one.
#[tokio::test]
async fn resolving_a_real_pendency_returns_it_to_pending_and_recomputes_the_work_order() {
    let db = mock()
        .append_query_results([[entity::work_order_item_entity::Model {
            purchase_order_id: Some(950),
            ..work_order_item_row(99, 42, 96, "AwaitingParts")
        }]]) // the real pendency
        .append_query_results([[work_order_row_with_status(96, 42, 10, "AwaitingParts")]]) // its work order
        .append_exec_results([inserted(99)]) // item update
        .append_query_results([[work_order_item_row(99, 42, 96, "Pending")]]) // item refetch
        .append_query_results([[work_order_item_row(99, 42, 96, "Pending")]]) // all items
        .append_exec_results([inserted(96)]) // work order update
        .append_query_results([[work_order_row_with_status(96, 42, 10, "Open")]]) // refetch
        .into_connection();

    let updated = run_with_user(Some(owner(42)), work_order_use_case(&db).resolve_purchase_pendency(99))
        .await
        .expect("resolving a real pendency succeeds");
    assert_eq!(updated.status, WorkOrderStatus::Open);
}

// ------------------------------------------------------- EPIC-MT-03-S01 maintenance plan
use business::domain::enums::MaintenancePlanStatus;
use business::domain::maintenance_plan::MaintenancePlan;
use business::gateway::maintenance_plan_gateway::MaintenancePlanGateway;
use business::use_cases::maintenance_plan_use_case::{
    MaintenancePlanUseCase, NOT_A_TENANT_VEHICLE as PLAN_NOT_A_TENANT_VEHICLE, OVERLAPPING_PLAN_EXISTS,
    WORK_ORDER_ALREADY_SCHEDULED, WORK_ORDER_WRONG_VEHICLE,
};

fn maintenance_plan_use_case(db: &DatabaseConnection) -> MaintenancePlanUseCase {
    MaintenancePlanUseCase::new(
        MaintenancePlanGateway::new(db.clone()),
        WorkOrderGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
    )
}

fn new_maintenance_plan(tenant_id: Option<i64>, date: NaiveDate) -> MaintenancePlan {
    MaintenancePlan {
        id: None,
        uuid: None,
        tenant_id,
        vehicle_id: 10,
        date,
        planned_start: None,
        planned_end: None,
        status: MaintenancePlanStatus::Scheduled,
        affects_schedule: false,
        origin: "Manual".into(),
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn maintenance_plan_row(
    id: i64,
    tenant_id: i64,
    vehicle_id: i64,
    date: NaiveDate,
    status: &str,
) -> entity::maintenance_plan_entity::Model {
    entity::maintenance_plan_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        vehicle_id,
        date,
        planned_start: None,
        planned_end: None,
        status: status.into(),
        affects_schedule: false,
        origin: "Manual".into(),
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

fn a_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 1).unwrap()
}

/// Same `DEF-FO-01` lesson every other allocation-style use case here
/// proves, for the plan's vehicle.
#[tokio::test]
async fn a_maintenance_plan_vehicle_must_belong_to_the_plans_own_tenant() {
    let unbound_admin = AuditUser {
        id: 1,
        email: "admin@example.com".into(),
        tenant_id: None,
        enforce_tenant: false,
    };
    let plan = new_maintenance_plan(Some(42), a_date());
    let db = mock()
        .append_query_results([[vehicle_row(10, 99, "ABC1D23", "Active")]])
        .into_connection();
    let refused = run_with_user(
        Some(unbound_admin),
        maintenance_plan_use_case(&db).create(plan, Vec::new()),
    )
    .await
    .expect_err("vehicle belongs to another tenant");
    assert_eq!(refused.message, PLAN_NOT_A_TENANT_VEHICLE);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

/// `TRM-234`: two overlapping maintenance windows for the same vehicle on
/// the same date are a data-entry error.
#[tokio::test]
async fn an_overlapping_active_plan_for_the_same_vehicle_and_date_is_refused() {
    let plan = new_maintenance_plan(Some(42), a_date());
    let db = mock()
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([[maintenance_plan_row(500, 42, 10, a_date(), "Scheduled")]])
        .into_connection();
    let refused = run_with_user(
        Some(owner(42)),
        maintenance_plan_use_case(&db).create(plan, Vec::new()),
    )
    .await
    .expect_err("an active overlapping plan already exists");
    assert_eq!(refused.message, OVERLAPPING_PLAN_EXISTS);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

/// `TRM-237`: a work order named for a plan must be this plan's own
/// vehicle's work order.
#[tokio::test]
async fn a_named_work_order_must_belong_to_the_plans_own_vehicle() {
    let plan = new_maintenance_plan(Some(42), a_date());
    let db = mock()
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([Vec::<entity::maintenance_plan_entity::Model>::new()]) // no overlap
        .append_query_results([[work_order_row(97, 42, 999)]]) // a different vehicle
        .into_connection();
    let refused = run_with_user(
        Some(owner(42)),
        maintenance_plan_use_case(&db).create(plan, vec![97]),
    )
    .await
    .expect_err("the work order belongs to a different vehicle");
    assert_eq!(refused.message, WORK_ORDER_WRONG_VEHICLE);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

/// `TRM-237`: a work order already scheduled into another *active* plan is
/// not available to a second one.
#[tokio::test]
async fn a_work_order_already_scheduled_into_an_active_plan_is_refused() {
    let plan = new_maintenance_plan(Some(42), a_date());
    let db = mock()
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([Vec::<entity::maintenance_plan_entity::Model>::new()])
        .append_query_results([[entity::work_order_entity::Model {
            maintenance_plan_id: Some(501),
            preventive_plan_id: None,
            ..work_order_row(97, 42, 10)
        }]])
        .append_query_results([[maintenance_plan_row(501, 42, 10, a_date(), "Scheduled")]]) // its current plan, still active
        .into_connection();
    let refused = run_with_user(
        Some(owner(42)),
        maintenance_plan_use_case(&db).create(plan, vec![97]),
    )
    .await
    .expect_err("the work order is already scheduled elsewhere");
    assert_eq!(refused.message, WORK_ORDER_ALREADY_SCHEDULED);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

/// `TRM-237`'s own point: a work order whose previous plan has since become
/// inactive is not locked out of being rescheduled.
#[tokio::test]
async fn a_valid_maintenance_plan_is_created_and_reschedules_a_work_order_from_an_inactive_plan() {
    let plan = new_maintenance_plan(Some(42), a_date());
    let db = mock()
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]]) // vehicle
        .append_query_results([Vec::<entity::maintenance_plan_entity::Model>::new()]) // no overlap
        .append_query_results([[entity::work_order_entity::Model {
            maintenance_plan_id: Some(501),
            preventive_plan_id: None,
            ..work_order_row(97, 42, 10)
        }]]) // the named work order, currently linked to plan 501
        .append_query_results([[maintenance_plan_row(501, 42, 10, a_date(), "Concluded")]]) // 501 is now inactive
        .append_exec_results([inserted(502)]) // insert the new plan
        .append_query_results([[maintenance_plan_row(502, 42, 10, a_date(), "Scheduled")]]) // refetch after insert
        .append_exec_results([inserted(97)]) // update the work order's maintenance_plan_id
        .append_query_results([[entity::work_order_entity::Model {
            maintenance_plan_id: Some(502),
            preventive_plan_id: None,
            ..work_order_row(97, 42, 10)
        }]]) // refetch after update
        .into_connection();

    let (saved_plan, linked) = run_with_user(
        Some(owner(42)),
        maintenance_plan_use_case(&db).create(plan, vec![97]),
    )
    .await
    .expect("a valid plan with a reschedulable work order is accepted");

    assert_eq!(saved_plan.id, Some(502));
    assert_eq!(linked.len(), 1);
    assert_eq!(linked[0].maintenance_plan_id, Some(502));
}

// ------------------------------------------------------- EPIC-MT-05-S01 service catalogues
use business::domain::priced_service::PricedService;
use business::domain::service_type::ServiceType;
use business::gateway::priced_service_gateway::PricedServiceGateway;
use business::gateway::service_type_gateway::ServiceTypeGateway;
use business::use_cases::priced_service_use_case::{
    NAME_REQUIRED as PRICED_SERVICE_NAME_REQUIRED, PricedServiceUseCase,
};
use business::use_cases::service_type_use_case::{
    CODE_REQUIRED, DUPLICATE_CODE, NAME_REQUIRED as SERVICE_TYPE_NAME_REQUIRED, ServiceTypeUseCase,
};

fn service_type_use_case(
    db: &DatabaseConnection,
) -> ServiceTypeUseCase<business::gateway::service_type_gateway::ServiceTypeGateway> {
    ServiceTypeUseCase::new(ServiceTypeGateway::new(db.clone()))
}

fn new_service_type(tenant_id: Option<i64>, code: &str, name: &str) -> ServiceType {
    ServiceType {
        id: None,
        uuid: None,
        tenant_id,
        code: code.into(),
        name: name.into(),
        category: None,
        active: true,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn service_type_row(id: i64, tenant_id: i64, code: &str) -> entity::service_type_entity::Model {
    entity::service_type_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        code: code.into(),
        name: "Brakes".into(),
        category: None,
        active: true,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

#[tokio::test]
async fn a_blank_service_type_code_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let service_type = new_service_type(Some(42), "   ", "Brakes");
    let err = run_with_user(Some(owner(42)), service_type_use_case(&db).create(service_type))
        .await
        .expect_err("a blank code is refused");
    assert_eq!(err.message, CODE_REQUIRED);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn a_blank_service_type_name_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let service_type = new_service_type(Some(42), "BRK", "   ");
    let err = run_with_user(Some(owner(42)), service_type_use_case(&db).create(service_type))
        .await
        .expect_err("a blank name is refused");
    assert_eq!(err.message, SERVICE_TYPE_NAME_REQUIRED);
    assert!(log(db).is_empty());
}

/// `HRMS-704`: same fail-fast shape `saving_a_duplicate_acronym_is_refused_
/// before_writing` proves for `Province`.
#[tokio::test]
async fn a_duplicate_service_type_code_is_refused_before_writing() {
    let db = mock()
        .append_query_results([[service_type_row(600, 42, "BRK")]])
        .into_connection();
    let service_type = new_service_type(Some(42), "BRK", "Brakes");
    let err = run_with_user(Some(owner(42)), service_type_use_case(&db).create(service_type))
        .await
        .expect_err("a duplicate code is refused");
    assert_eq!(err.message, DUPLICATE_CODE);
    assert_eq!(log(db).len(), 1, "only the lookup ran; nothing was written");
}

#[tokio::test]
async fn saving_a_new_service_type_inserts_once() {
    let db = mock()
        .append_query_results([Vec::<entity::service_type_entity::Model>::new()]) // no existing code
        .append_exec_results([inserted(600)])
        .append_query_results([[service_type_row(600, 42, "BRK")]])
        .into_connection();
    let service_type = new_service_type(Some(42), "BRK", "Brakes");
    let saved = run_with_user(Some(owner(42)), service_type_use_case(&db).create(service_type))
        .await
        .expect("a valid service type is accepted");
    assert_eq!(saved.id, Some(600));
}

fn priced_service_use_case(
    db: &DatabaseConnection,
) -> PricedServiceUseCase<business::gateway::priced_service_gateway::PricedServiceGateway> {
    PricedServiceUseCase::new(PricedServiceGateway::new(db.clone()))
}

fn new_priced_service(tenant_id: Option<i64>, name: &str) -> PricedService {
    PricedService {
        id: None,
        uuid: None,
        tenant_id,
        name: name.into(),
        category: None,
        default_value_cents: Some(15_000),
        observation: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn priced_service_row(id: i64, tenant_id: i64) -> entity::priced_service_entity::Model {
    entity::priced_service_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        name: "Oil change".into(),
        category: None,
        default_value_cents: Some(15_000),
        observation: None,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

#[tokio::test]
async fn a_blank_priced_service_name_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let priced_service = new_priced_service(Some(42), "   ");
    let err = run_with_user(Some(owner(42)), priced_service_use_case(&db).create(priced_service))
        .await
        .expect_err("a blank name is refused");
    assert_eq!(err.message, PRICED_SERVICE_NAME_REQUIRED);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn saving_a_new_priced_service_inserts_once() {
    let db = mock()
        .append_exec_results([inserted(700)])
        .append_query_results([[priced_service_row(700, 42)]])
        .into_connection();
    let priced_service = new_priced_service(Some(42), "Oil change");
    let saved = run_with_user(Some(owner(42)), priced_service_use_case(&db).create(priced_service))
        .await
        .expect("a valid priced service is accepted");
    assert_eq!(saved.id, Some(700));
}

// ------------------------------------------------------------- EPIC-SP-01-S01 parts catalogue
use business::domain::part::Part;
use business::gateway::part_gateway::PartGateway;
use business::use_cases::part_use_case::{
    NAME_REQUIRED as PART_NAME_REQUIRED, NEGATIVE_MINIMUM_STOCK, PartUseCase, UNIT_REQUIRED,
};

fn part_use_case(db: &DatabaseConnection) -> PartUseCase<PartGateway> {
    PartUseCase::new(PartGateway::new(db.clone()))
}

fn new_part(tenant_id: Option<i64>, name: &str, unit: &str, minimum_stock: f64) -> Part {
    Part {
        id: None,
        uuid: None,
        tenant_id,
        name: name.into(),
        category: None,
        application: None,
        minimum_stock,
        unit: unit.into(),
        unit_value_cents: Some(5_000),
        default_supplier: None,
        location: None,
        observation: None,
        moving_average_cost_cents: None,
        last_purchase_price_cents: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn part_row(id: i64, tenant_id: i64) -> entity::part_entity::Model {
    entity::part_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        name: "Brake pad".into(),
        category: None,
        application: None,
        minimum_stock: 4.0,
        unit: "un".into(),
        unit_value_cents: Some(5_000),
        default_supplier: None,
        location: None,
        observation: None,
        moving_average_cost_cents: None,
        last_purchase_price_cents: None,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

#[tokio::test]
async fn a_blank_part_name_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let part = new_part(Some(42), "   ", "un", 0.0);
    let err = run_with_user(Some(owner(42)), part_use_case(&db).create(part))
        .await
        .expect_err("a blank name is refused");
    assert_eq!(err.message, PART_NAME_REQUIRED);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn a_blank_part_unit_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let part = new_part(Some(42), "Brake pad", "   ", 0.0);
    let err = run_with_user(Some(owner(42)), part_use_case(&db).create(part))
        .await
        .expect_err("a blank unit is refused");
    assert_eq!(err.message, UNIT_REQUIRED);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn a_negative_minimum_stock_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let part = new_part(Some(42), "Brake pad", "un", -1.0);
    let err = run_with_user(Some(owner(42)), part_use_case(&db).create(part))
        .await
        .expect_err("a negative minimum stock is refused");
    assert_eq!(err.message, NEGATIVE_MINIMUM_STOCK);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn saving_a_new_part_inserts_once() {
    let db = mock()
        .append_exec_results([inserted(800)])
        .append_query_results([[part_row(800, 42)]])
        .into_connection();
    let part = new_part(Some(42), "Brake pad", "un", 4.0);
    let saved = run_with_user(Some(owner(42)), part_use_case(&db).create(part))
        .await
        .expect("a valid part is accepted");
    assert_eq!(saved.id, Some(800));
}

// ------------------------------------------------------------- EPIC-SP-02-S01 stock ledger
use business::gateway::stock_movement_gateway::StockMovementGateway;
use business::use_cases::stock_movement_use_case::{
    NON_NEGATIVE_BALANCE_REQUIRED, PART_NOT_FOUND as MOVEMENT_PART_NOT_FOUND, QUANTITY_MUST_BE_POSITIVE,
    StockMovementUseCase, VALUE_REQUIRED,
};

fn stock_movement_use_case(db: &DatabaseConnection) -> StockMovementUseCase {
    StockMovementUseCase::new(StockMovementGateway::new(db.clone()), PartGateway::new(db.clone()))
}

fn part_row_with_cost(id: i64, tenant_id: i64, moving_average_cost_cents: Option<i64>) -> entity::part_entity::Model {
    entity::part_entity::Model {
        moving_average_cost_cents,
        ..part_row(id, tenant_id)
    }
}

fn stock_movement_row(
    id: i64,
    tenant_id: i64,
    part_id: i64,
    movement_type: &str,
    quantity: f64,
) -> entity::stock_movement_entity::Model {
    entity::stock_movement_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        part_id,
        movement_type: movement_type.into(),
        quantity,
        unit_value_cents: Some(5_000),
        total_value_cents: Some(50_000),
        cost_source: "Informed".into(),
        supplier: None,
        invoice_number: None,
        entry_date: None,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

#[tokio::test]
async fn a_non_positive_entry_quantity_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let err = run_with_user(
        Some(owner(42)),
        stock_movement_use_case(&db).record_entry(800, 0.0, Some(5_000), None, None, None, None),
    )
    .await
    .expect_err("a zero quantity is refused");
    assert_eq!(err.message, QUANTITY_MUST_BE_POSITIVE);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn an_entry_with_neither_unit_nor_total_value_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let err = run_with_user(
        Some(owner(42)),
        stock_movement_use_case(&db).record_entry(800, 10.0, None, None, None, None, None),
    )
    .await
    .expect_err("neither value is refused");
    assert_eq!(err.message, VALUE_REQUIRED);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn an_entry_against_an_unknown_part_is_refused_after_one_lookup() {
    let db = mock()
        .append_query_results([Vec::<entity::part_entity::Model>::new()])
        .into_connection();
    let err = run_with_user(
        Some(owner(42)),
        stock_movement_use_case(&db).record_entry(800, 10.0, Some(5_000), None, None, None, None),
    )
    .await
    .expect_err("an unknown part is refused");
    assert_eq!(err.message, MOVEMENT_PART_NOT_FOUND);
    assert_eq!(log(db).len(), 1, "only the part lookup ran; nothing was written");
}

/// `TRM-602`/`608`/`610`/`611`: a fresh part (no prior movements, no prior
/// average) takes the entry's own unit value as its first moving average and
/// last purchase price, derived here from `totalValueCents` (`TRM-611`).
#[tokio::test]
async fn a_first_entry_seeds_the_moving_average_from_the_entered_value() {
    let db = mock()
        .append_query_results([[part_row_with_cost(800, 42, None)]]) // find part
        .append_query_results([Vec::<entity::stock_movement_entity::Model>::new()]) // current_stock: no movements yet
        .append_exec_results([inserted(900)]) // insert the movement
        .append_query_results([[stock_movement_row(900, 42, 800, "Entry", 10.0)]]) // refetch
        .append_exec_results([inserted(800)]) // update the part's cost fields
        .append_query_results([[part_row_with_cost(800, 42, Some(5_000))]]) // refetch
        .into_connection();

    let saved = run_with_user(
        Some(owner(42)),
        stock_movement_use_case(&db).record_entry(800, 10.0, None, Some(50_000), None, None, None),
    )
    .await
    .expect("a valid entry is accepted");
    assert_eq!(saved.id, Some(900));
    assert_eq!(saved.quantity, 10.0);
}

#[tokio::test]
async fn a_negative_adjustment_balance_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let err = run_with_user(Some(owner(42)), stock_movement_use_case(&db).adjust(800, -1.0))
        .await
        .expect_err("a negative balance is refused");
    assert_eq!(err.message, NON_NEGATIVE_BALANCE_REQUIRED);
    assert!(log(db).is_empty());
}

/// `TRM-604`/`617`: a single explicit adjustment movement for the
/// difference between the ledger's current sum and the counted balance.
#[tokio::test]
async fn an_adjustment_records_the_difference_from_current_stock() {
    let db = mock()
        .append_query_results([[part_row(800, 42)]]) // find part
        .append_query_results([Vec::<entity::stock_movement_entity::Model>::new()]) // current_stock: 0
        .append_exec_results([inserted(901)]) // insert the adjustment
        .append_query_results([[stock_movement_row(901, 42, 800, "Adjustment", 5.0)]]) // refetch
        .into_connection();

    let saved = run_with_user(Some(owner(42)), stock_movement_use_case(&db).adjust(800, 5.0))
        .await
        .expect("a valid adjustment is accepted");
    assert_eq!(saved.id, Some(901));
}

// ------------------------------------------------------------- EPIC-SP-03-S01 purchase orders
use business::domain::purchase_order::PurchaseOrder;
use business::gateway::purchase_order_gateway::PurchaseOrderGateway;
use business::use_cases::purchase_order_use_case::{
    DUPLICATE_ACTIVE_PURCHASE_ORDER, PART_NOT_FOUND as PO_PART_NOT_FOUND,
    PENDENCY_REQUIRES_WORK_ORDER, PURCHASE_ORDER_NOT_FOUND, PurchaseOrderUseCase,
    QUANTITY_MUST_BE_POSITIVE as PO_QUANTITY_MUST_BE_POSITIVE, WRONG_STATUS_FOR_TRANSITION,
};

fn purchase_order_use_case(db: &DatabaseConnection) -> PurchaseOrderUseCase {
    PurchaseOrderUseCase::new(
        PurchaseOrderGateway::new(db.clone()),
        PartGateway::new(db.clone()),
        work_order_use_case(db),
    )
}

fn new_purchase_order(tenant_id: Option<i64>, part_id: i64, quantity: f64, work_order_id: Option<i64>) -> PurchaseOrder {
    PurchaseOrder {
        id: None,
        uuid: None,
        tenant_id,
        part_id,
        quantity,
        suggested_supplier: None,
        observation: None,
        status: Default::default(),
        work_order_id,
        work_order_item_id: None,
        vehicle_id: None,
        ordered_at: None,
        expected_delivery_date: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn purchase_order_row(id: i64, tenant_id: i64, part_id: i64, status: &str) -> entity::purchase_order_entity::Model {
    entity::purchase_order_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        part_id,
        quantity: 4.0,
        suggested_supplier: None,
        observation: None,
        status: status.into(),
        work_order_id: Some(94),
        work_order_item_id: None,
        vehicle_id: None,
        ordered_at: None,
        expected_delivery_date: None,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

#[tokio::test]
async fn a_non_positive_purchase_order_quantity_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let purchase_order = new_purchase_order(Some(42), 800, 0.0, None);
    let err = run_with_user(Some(owner(42)), purchase_order_use_case(&db).create(purchase_order))
        .await
        .expect_err("a zero quantity is refused");
    assert_eq!(err.message, PO_QUANTITY_MUST_BE_POSITIVE);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn a_purchase_order_against_an_unknown_part_is_refused_after_one_lookup() {
    let db = mock()
        .append_query_results([Vec::<entity::part_entity::Model>::new()])
        .into_connection();
    let purchase_order = new_purchase_order(Some(42), 800, 4.0, None);
    let err = run_with_user(Some(owner(42)), purchase_order_use_case(&db).create(purchase_order))
        .await
        .expect_err("an unknown part is refused");
    assert_eq!(err.message, PO_PART_NOT_FOUND);
    assert_eq!(log(db).len(), 1, "only the part lookup ran; nothing was written");
}

/// `TRM-642`: a second active order for the same part on the same work order
/// is refused after the part and duplicate lookups, before any write.
#[tokio::test]
async fn a_duplicate_active_purchase_order_for_the_same_part_and_work_order_is_refused() {
    let db = mock()
        .append_query_results([[part_row(800, 42)]]) // part exists
        .append_query_results([[purchase_order_row(950, 42, 800, "Requested")]]) // an existing active order
        .into_connection();
    let purchase_order = new_purchase_order(Some(42), 800, 4.0, Some(94));
    let err = run_with_user(Some(owner(42)), purchase_order_use_case(&db).create(purchase_order))
        .await
        .expect_err("a duplicate active order is refused");
    assert_eq!(err.message, DUPLICATE_ACTIVE_PURCHASE_ORDER);
    assert_eq!(log(db).len(), 2, "only the two lookups ran; nothing was written");
}

#[tokio::test]
async fn saving_a_new_purchase_order_inserts_once() {
    let db = mock()
        .append_query_results([[part_row(800, 42)]]) // part exists
        .append_exec_results([inserted(950)]) // insert
        .append_query_results([[entity::purchase_order_entity::Model {
            work_order_id: None,
            ..purchase_order_row(950, 42, 800, "Requested")
        }]]) // refetch: no work order named, so no pendency wiring runs
        .into_connection();
    let purchase_order = new_purchase_order(Some(42), 800, 4.0, None);
    let saved = run_with_user(Some(owner(42)), purchase_order_use_case(&db).create(purchase_order))
        .await
        .expect("a valid purchase order is accepted");
    assert_eq!(saved.id, Some(950));
}

#[tokio::test]
async fn marking_a_requested_order_ordered_succeeds() {
    let db = mock()
        .append_query_results([[purchase_order_row(950, 42, 800, "Requested")]]) // loaded
        .append_exec_results([inserted(950)]) // update
        .append_query_results([[purchase_order_row(950, 42, 800, "Ordered")]]) // refetch
        .into_connection();
    let saved = run_with_user(Some(owner(42)), purchase_order_use_case(&db).mark_ordered(950, None, None))
        .await
        .expect("a requested order can be marked ordered");
    assert_eq!(saved.status, business::domain::enums::PurchaseOrderStatus::Ordered);
}

#[tokio::test]
async fn marking_an_already_ordered_order_ordered_again_is_refused() {
    let db = mock()
        .append_query_results([[purchase_order_row(950, 42, 800, "Ordered")]])
        .into_connection();
    let err = run_with_user(Some(owner(42)), purchase_order_use_case(&db).mark_ordered(950, None, None))
        .await
        .expect_err("only a Requested order may be marked ordered");
    assert_eq!(err.message, WRONG_STATUS_FOR_TRANSITION);
}

#[tokio::test]
async fn cancelling_an_already_cancelled_purchase_order_is_a_no_op() {
    let db = mock()
        .append_query_results([[purchase_order_row(950, 42, 800, "Cancelled")]])
        .into_connection();
    let saved = run_with_user(Some(owner(42)), purchase_order_use_case(&db).cancel(950))
        .await
        .expect("cancelling an already-cancelled order succeeds");
    assert_eq!(saved.status, business::domain::enums::PurchaseOrderStatus::Cancelled);
    assert_eq!(log(db).len(), 1, "only the load ran; nothing was written");
}

#[tokio::test]
async fn a_missing_purchase_order_is_reported_not_silently_ignored() {
    let db = mock()
        .append_query_results([Vec::<entity::purchase_order_entity::Model>::new()])
        .into_connection();
    let err = run_with_user(Some(owner(42)), purchase_order_use_case(&db).cancel(950))
        .await
        .expect_err("a missing purchase order is reported");
    assert_eq!(err.message, PURCHASE_ORDER_NOT_FOUND);
}

#[tokio::test]
async fn a_pendency_named_without_its_own_work_order_is_refused_before_any_query() {
    let db = mock().into_connection();
    let mut purchase_order = new_purchase_order(Some(42), 800, 4.0, None);
    purchase_order.work_order_item_id = Some(97);
    let err = run_with_user(Some(owner(42)), purchase_order_use_case(&db).create(purchase_order))
        .await
        .expect_err("a pendency named with no work order is refused");
    assert_eq!(err.message, PENDENCY_REQUIRES_WORK_ORDER);
    assert!(log(db).is_empty());
}

/// `TRM-644`/`646`: raising a purchase order against a named pendency links
/// that item and moves the work order to `AwaitingParts`.
#[tokio::test]
async fn raising_a_purchase_order_against_an_existing_pendency_links_it() {
    let db = mock()
        .append_query_results([[part_row(800, 42)]]) // part exists
        .append_query_results([Vec::<entity::purchase_order_entity::Model>::new()]) // no duplicate
        .append_exec_results([inserted(950)]) // insert the purchase order
        .append_query_results([[entity::purchase_order_entity::Model {
            work_order_id: Some(96),
            work_order_item_id: None,
            ..purchase_order_row(950, 42, 800, "Requested")
        }]]) // refetch, pendency not yet linked
        // -- mark_item_awaiting_parts(96, 97, 950) --
        .append_query_results([[work_order_row_with_status(96, 42, 10, "Open")]])
        .append_query_results([[work_order_item_row(97, 42, 96, "Pending")]])
        .append_exec_results([inserted(97)])
        .append_query_results([[entity::work_order_item_entity::Model {
            status: "AwaitingParts".into(),
            purchase_order_id: Some(950),
            ..work_order_item_row(97, 42, 96, "AwaitingParts")
        }]])
        .append_query_results([[entity::work_order_item_entity::Model {
            status: "AwaitingParts".into(),
            purchase_order_id: Some(950),
            ..work_order_item_row(97, 42, 96, "AwaitingParts")
        }]])
        .append_exec_results([inserted(96)])
        .append_query_results([[work_order_row_with_status(96, 42, 10, "AwaitingParts")]])
        // -- persist the purchase order's own work_order_item_id --
        .append_exec_results([inserted(950)])
        .append_query_results([[entity::purchase_order_entity::Model {
            work_order_id: Some(96),
            work_order_item_id: Some(97),
            ..purchase_order_row(950, 42, 800, "Requested")
        }]]) // final refetch: pendency now linked
        .into_connection();

    let mut purchase_order = new_purchase_order(Some(42), 800, 4.0, Some(96));
    purchase_order.work_order_item_id = Some(97);
    let saved = run_with_user(Some(owner(42)), purchase_order_use_case(&db).create(purchase_order))
        .await
        .expect("linking an existing pendency succeeds");
    assert_eq!(saved.id, Some(950));
}

/// `TRM-645`/`646`: raising a purchase order against a work order with no
/// named pendency opens a synthetic placeholder item instead.
#[tokio::test]
async fn raising_a_purchase_order_with_no_named_pendency_opens_a_synthetic_placeholder() {
    let db = mock()
        .append_query_results([[part_row(800, 42)]]) // part exists
        .append_query_results([Vec::<entity::purchase_order_entity::Model>::new()]) // no duplicate
        .append_exec_results([inserted(950)]) // insert the purchase order
        .append_query_results([[entity::purchase_order_entity::Model {
            work_order_id: Some(96),
            work_order_item_id: None,
            ..purchase_order_row(950, 42, 800, "Requested")
        }]]) // refetch
        // -- add_item(96, placeholder) --
        .append_query_results([[work_order_row_with_status(96, 42, 10, "Open")]])
        .append_exec_results([inserted(98)])
        .append_query_results([[entity::work_order_item_entity::Model {
            is_purchase_placeholder: true,
            preventive_plan_id: None,
            purchase_order_id: Some(950),
            ..work_order_item_row(98, 42, 96, "AwaitingParts")
        }]])
        .append_query_results([[entity::work_order_item_entity::Model {
            is_purchase_placeholder: true,
            preventive_plan_id: None,
            purchase_order_id: Some(950),
            ..work_order_item_row(98, 42, 96, "AwaitingParts")
        }]])
        .append_exec_results([inserted(96)])
        .append_query_results([[work_order_row_with_status(96, 42, 10, "AwaitingParts")]])
        // -- persist the purchase order's own work_order_item_id --
        .append_exec_results([inserted(950)])
        .append_query_results([[entity::purchase_order_entity::Model {
            work_order_id: Some(96),
            work_order_item_id: Some(98),
            ..purchase_order_row(950, 42, 800, "Requested")
        }]]) // final refetch: synthetic pendency now linked
        .into_connection();

    let purchase_order = new_purchase_order(Some(42), 800, 4.0, Some(96));
    let saved = run_with_user(Some(owner(42)), purchase_order_use_case(&db).create(purchase_order))
        .await
        .expect("opening a synthetic placeholder pendency succeeds");
    assert_eq!(saved.id, Some(950));
}

/// `TRM-647`/`648`/`649`: receiving a purchase order resolves its linked
/// pendency.
#[tokio::test]
async fn marking_a_purchase_order_purchased_resolves_its_linked_pendency() {
    let db = mock()
        .append_query_results([[entity::purchase_order_entity::Model {
            work_order_item_id: Some(98),
            ..purchase_order_row(950, 42, 800, "Ordered")
        }]]) // loaded
        .append_exec_results([inserted(950)]) // status -> Purchased
        .append_query_results([[entity::purchase_order_entity::Model {
            work_order_item_id: Some(98),
            status: "Purchased".into(),
            ..purchase_order_row(950, 42, 800, "Purchased")
        }]]) // refetch
        // -- resolve_purchase_pendency(98) --
        .append_query_results([[entity::work_order_item_entity::Model {
            is_purchase_placeholder: true,
            preventive_plan_id: None,
            purchase_order_id: Some(950),
            ..work_order_item_row(98, 42, 96, "AwaitingParts")
        }]])
        .append_query_results([[work_order_row_with_status(96, 42, 10, "AwaitingParts")]])
        .append_exec_results([inserted(98)])
        .append_query_results([[entity::work_order_item_entity::Model {
            is_purchase_placeholder: true,
            preventive_plan_id: None,
            purchase_order_id: Some(950),
            ..work_order_item_row(98, 42, 96, "Resolved")
        }]])
        .append_query_results([[entity::work_order_item_entity::Model {
            is_purchase_placeholder: true,
            preventive_plan_id: None,
            purchase_order_id: Some(950),
            ..work_order_item_row(98, 42, 96, "Resolved")
        }]])
        .into_connection();

    let saved = run_with_user(Some(owner(42)), purchase_order_use_case(&db).mark_purchased(950))
        .await
        .expect("receiving a linked purchase order succeeds");
    assert_eq!(saved.status, business::domain::enums::PurchaseOrderStatus::Purchased);
}

// ------------------------------------------------------- EPIC-MT-02-S01 work order postings
use business::gateway::work_order_posting_gateway::WorkOrderPostingGateway;
use business::use_cases::work_order_posting_use_case::{
    INSUFFICIENT_STOCK, PART_NOT_FOUND as ISSUE_PART_NOT_FOUND, QUANTITY_MUST_BE_POSITIVE as ISSUE_QUANTITY_MUST_BE_POSITIVE,
    WORK_ORDER_NOT_FOUND as ISSUE_WORK_ORDER_NOT_FOUND, WorkOrderPostingUseCase,
};

fn work_order_posting_use_case(db: &DatabaseConnection) -> WorkOrderPostingUseCase {
    WorkOrderPostingUseCase::new(
        WorkOrderPostingGateway::new(db.clone()),
        WorkOrderGateway::new(db.clone()),
        WorkOrderItemGateway::new(db.clone()),
        PartGateway::new(db.clone()),
        StockMovementGateway::new(db.clone()),
        StockMovementUseCase::new(StockMovementGateway::new(db.clone()), PartGateway::new(db.clone())),
    )
}

#[allow(clippy::too_many_arguments)]
fn work_order_posting_row(
    id: i64,
    tenant_id: i64,
    work_order_id: i64,
    work_order_item_id: Option<i64>,
    part_id: i64,
    quantity: f64,
    unit_value_cents: i64,
    total_value_cents: i64,
    cost_source: &str,
) -> entity::work_order_posting_entity::Model {
    entity::work_order_posting_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        work_order_id,
        work_order_item_id,
        part_id,
        quantity,
        unit_value_cents,
        total_value_cents,
        cost_source: cost_source.into(),
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

#[tokio::test]
async fn issuing_a_non_positive_quantity_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let err = run_with_user(
        Some(owner(42)),
        work_order_posting_use_case(&db).issue(96, None, 800, 0.0),
    )
    .await
    .expect_err("a non-positive quantity is refused");
    assert_eq!(err.message, ISSUE_QUANTITY_MUST_BE_POSITIVE);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn issuing_against_an_unknown_work_order_is_refused_after_one_lookup() {
    let db = mock()
        .append_query_results([Vec::<entity::work_order_entity::Model>::new()])
        .into_connection();
    let err = run_with_user(
        Some(owner(42)),
        work_order_posting_use_case(&db).issue(96, None, 800, 3.0),
    )
    .await
    .expect_err("an unknown work order is refused");
    assert_eq!(err.message, ISSUE_WORK_ORDER_NOT_FOUND);
    assert_eq!(log(db).len(), 1, "only the work order lookup ran; nothing was written");
}

#[tokio::test]
async fn issuing_against_an_unknown_part_is_refused_after_two_lookups() {
    let db = mock()
        .append_query_results([[work_order_row(96, 42, 10)]])
        .append_query_results([Vec::<entity::part_entity::Model>::new()])
        .into_connection();
    let err = run_with_user(
        Some(owner(42)),
        work_order_posting_use_case(&db).issue(96, None, 800, 3.0),
    )
    .await
    .expect_err("an unknown part is refused");
    assert_eq!(err.message, ISSUE_PART_NOT_FOUND);
    assert_eq!(log(db).len(), 2, "the work order and part lookups ran; nothing was written");
}

/// `TRM-613`: "refuse to issue a part to a work order when the recorded
/// stock is below the quantity requested."
#[tokio::test]
async fn issuing_more_than_the_current_stock_is_refused() {
    let db = mock()
        .append_query_results([[work_order_row(96, 42, 10)]]) // work order
        .append_query_results([[part_row(800, 42)]]) // part
        .append_query_results([[stock_movement_row(900, 42, 800, "Entry", 1.0)]]) // current_stock: 1
        .into_connection();
    let err = run_with_user(
        Some(owner(42)),
        work_order_posting_use_case(&db).issue(96, None, 800, 3.0),
    )
    .await
    .expect_err("insufficient stock is refused");
    assert_eq!(err.message, INSUFFICIENT_STOCK);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

/// `TRM-609`/`613`/`614`: a valid issue writes a negative-quantity `Issue`
/// stock movement and a costed posting in one transaction, costed by
/// `TRM-608`'s precedence -- here the part's moving average, which outranks
/// its registered unit value.
#[tokio::test]
async fn issuing_a_part_writes_a_stock_movement_and_a_costed_posting() {
    let db = mock()
        .append_query_results([[work_order_row(96, 42, 10)]]) // work order
        .append_query_results([[part_row_with_cost(800, 42, Some(7_500))]]) // part
        .append_query_results([[stock_movement_row(900, 42, 800, "Entry", 10.0)]]) // current_stock: 10
        .append_exec_results([inserted(901)]) // insert the Issue movement
        .append_query_results([[stock_movement_row(901, 42, 800, "Issue", -3.0)]]) // refetch
        .append_exec_results([inserted(500)]) // insert the posting
        .append_query_results([[work_order_posting_row(500, 42, 96, None, 800, 3.0, 7_500, 22_500, "MovingAverageCost")]]) // refetch
        .into_connection();

    let saved = run_with_user(
        Some(owner(42)),
        work_order_posting_use_case(&db).issue(96, None, 800, 3.0),
    )
    .await
    .expect("a valid issue is accepted");
    assert_eq!(saved.quantity, 3.0);
    assert_eq!(saved.unit_value_cents, 7_500);
    assert_eq!(saved.total_value_cents, 22_500);
    assert_eq!(saved.cost_source, business::domain::enums::CostSource::MovingAverageCost);

    let sql = format!("{:?}", log(db));
    assert!(sql.contains("-3"), "the ledger movement must carry the negative issued quantity: {sql}");
}

// ------------------------------------------------------- EPIC-SP-04-S01 vehicle expenses
use business::domain::enums::ExpenseOrigin;
use business::domain::vehicle_expense::VehicleExpense;
use business::gateway::vehicle_expense_gateway::VehicleExpenseGateway;
use business::use_cases::vehicle_expense_use_case::{
    CATEGORY_REQUIRED, COMPETENCE_PERIOD_REQUIRED, DUPLICATE_EXPENSE_INVOICE,
    VALUE_MUST_BE_POSITIVE as EXPENSE_VALUE_MUST_BE_POSITIVE, VEHICLE_NOT_FOUND as EXPENSE_VEHICLE_NOT_FOUND,
    VehicleExpenseUseCase,
};

fn vehicle_expense_use_case(db: &DatabaseConnection) -> VehicleExpenseUseCase {
    VehicleExpenseUseCase::new(VehicleExpenseGateway::new(db.clone()), VehicleGateway::new(db.clone()))
}

fn new_vehicle_expense(tenant_id: Option<i64>) -> VehicleExpense {
    VehicleExpense {
        id: None,
        uuid: None,
        tenant_id,
        vehicle_id: 10,
        category: "Tolls".into(),
        competence_period: "2026-09".into(),
        issue_date: chrono::NaiveDate::from_ymd_opt(2026, 9, 29).unwrap(),
        supplier: None,
        invoice_number: Some("INV-1".into()),
        description: None,
        value_cents: 5_000,
        origin: ExpenseOrigin::Manual,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn vehicle_expense_row(
    id: i64,
    tenant_id: i64,
    vehicle_id: i64,
    category: &str,
    invoice_number: Option<&str>,
) -> entity::vehicle_expense_entity::Model {
    entity::vehicle_expense_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        vehicle_id,
        category: category.into(),
        competence_period: "2026-09".into(),
        issue_date: chrono::NaiveDate::from_ymd_opt(2026, 9, 29).unwrap(),
        supplier: None,
        invoice_number: invoice_number.map(|s| s.into()),
        description: None,
        value_cents: 5_000,
        origin: "Manual".into(),
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

#[tokio::test]
async fn a_blank_expense_category_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let expense = VehicleExpense {
        category: "   ".into(),
        ..new_vehicle_expense(Some(42))
    };
    let err = run_with_user(Some(owner(42)), vehicle_expense_use_case(&db).create(expense))
        .await
        .expect_err("a blank category is refused");
    assert_eq!(err.message, CATEGORY_REQUIRED);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn a_blank_competence_period_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let expense = VehicleExpense {
        competence_period: "   ".into(),
        ..new_vehicle_expense(Some(42))
    };
    let err = run_with_user(Some(owner(42)), vehicle_expense_use_case(&db).create(expense))
        .await
        .expect_err("a blank competence period is refused");
    assert_eq!(err.message, COMPETENCE_PERIOD_REQUIRED);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn a_non_positive_expense_value_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let expense = VehicleExpense {
        value_cents: 0,
        ..new_vehicle_expense(Some(42))
    };
    let err = run_with_user(Some(owner(42)), vehicle_expense_use_case(&db).create(expense))
        .await
        .expect_err("a non-positive value is refused");
    assert_eq!(err.message, EXPENSE_VALUE_MUST_BE_POSITIVE);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn an_expense_against_a_vehicle_of_another_tenant_is_refused() {
    let db = mock()
        .append_query_results([[vehicle_row(10, 99, "ABC1D23", "Active")]])
        .into_connection();
    let err = run_with_user(
        Some(owner(42)),
        vehicle_expense_use_case(&db).create(new_vehicle_expense(Some(42))),
    )
    .await
    .expect_err("a vehicle from another tenant is refused");
    assert_eq!(err.message, EXPENSE_VEHICLE_NOT_FOUND);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

/// `TRM-661`: "one invoice number produces at most one live expense per
/// vehicle and category."
#[tokio::test]
async fn a_duplicate_invoice_number_in_the_same_category_is_refused() {
    let db = mock()
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]]) // vehicle
        .append_query_results([[vehicle_expense_row(900, 42, 10, "Tolls", Some("INV-1"))]]) // duplicate check
        .into_connection();
    let err = run_with_user(
        Some(owner(42)),
        vehicle_expense_use_case(&db).create(new_vehicle_expense(Some(42))),
    )
    .await
    .expect_err("a duplicate invoice number in the same category is refused");
    assert_eq!(err.message, DUPLICATE_EXPENSE_INVOICE);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

#[tokio::test]
async fn a_valid_vehicle_expense_is_accepted() {
    let db = mock()
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]]) // vehicle
        .append_query_results([Vec::<entity::vehicle_expense_entity::Model>::new()]) // duplicate check: none
        .append_exec_results([inserted(900)])
        .append_query_results([[vehicle_expense_row(900, 42, 10, "Tolls", Some("INV-1"))]])
        .into_connection();
    let saved = run_with_user(
        Some(owner(42)),
        vehicle_expense_use_case(&db).create(new_vehicle_expense(Some(42))),
    )
    .await
    .expect("a valid vehicle expense is accepted");
    assert_eq!(saved.id, Some(900));
}

// ------------------------------------------------------- EPIC-FU-01-S01 fuel ledger
use business::domain::enums::FuelEntryOrigin;
use business::domain::fuel_entry::FuelEntry;
use business::gateway::fuel_entry_gateway::FuelEntryGateway;
use business::use_cases::fuel_entry_use_case::{
    FuelEntryUseCase, VALUE_MUST_NOT_BE_NEGATIVE, VEHICLE_NOT_FOUND as FUEL_VEHICLE_NOT_FOUND,
    VOLUME_MUST_BE_POSITIVE,
};

fn fuel_entry_use_case(db: &DatabaseConnection) -> FuelEntryUseCase {
    FuelEntryUseCase::new(FuelEntryGateway::new(db.clone()), VehicleGateway::new(db.clone()), business::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway::new(db.clone()))
}

fn new_fuel_entry(tenant_id: Option<i64>) -> FuelEntry {
    FuelEntry {
        id: None,
        uuid: None,
        tenant_id,
        vehicle_id: 10,
        recorded_at: at().naive_utc(),
        volume_liters: 50.0,
        value_cents: 30_000,
        odometer_km: Some(123_456.0),
        station: Some("Posto Ipiranga".into()),
        full_tank: true,
        origin: FuelEntryOrigin::Manual,
        provider_transaction_id: None,
        reported_by_user_id: None,
        odometer_override_note: None,
        provider_confirmed_at: None,
        deleted_at: None,
        unified_into_id: None,
        unification_note: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn fuel_entry_row(id: i64, tenant_id: i64, vehicle_id: i64) -> entity::fuel_entry_entity::Model {
    entity::fuel_entry_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        vehicle_id,
        recorded_at: at(),
        volume_liters: 50.0,
        value_cents: 30_000,
        odometer_km: Some(123_456.0),
        station: Some("Posto Ipiranga".into()),
        full_tank: true,
        origin: "Manual".into(),
        provider_transaction_id: None,
        reported_by_user_id: None,
        odometer_override_note: None,
        provider_confirmed_at: None,
        deleted_at: None,
        unified_into_id: None,
        unification_note: None,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

#[tokio::test]
async fn a_non_positive_fuel_volume_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let entry = FuelEntry {
        volume_liters: 0.0,
        ..new_fuel_entry(Some(42))
    };
    let err = run_with_user(Some(owner(42)), fuel_entry_use_case(&db).create(entry))
        .await
        .expect_err("a non-positive volume is refused");
    assert_eq!(err.message, VOLUME_MUST_BE_POSITIVE);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn a_negative_fuel_value_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let entry = FuelEntry {
        value_cents: -1,
        ..new_fuel_entry(Some(42))
    };
    let err = run_with_user(Some(owner(42)), fuel_entry_use_case(&db).create(entry))
        .await
        .expect_err("a negative value is refused");
    assert_eq!(err.message, VALUE_MUST_NOT_BE_NEGATIVE);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn a_fuel_entry_against_a_vehicle_of_another_tenant_is_refused() {
    let db = mock()
        .append_query_results([[vehicle_row(10, 99, "ABC1D23", "Active")]])
        .into_connection();
    let err = run_with_user(Some(owner(42)), fuel_entry_use_case(&db).create(new_fuel_entry(Some(42))))
        .await
        .expect_err("a vehicle from another tenant is refused");
    assert_eq!(err.message, FUEL_VEHICLE_NOT_FOUND);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

#[tokio::test]
async fn a_valid_fuel_entry_is_accepted() {
    let db = mock()
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]]) // vehicle
        .append_exec_results([inserted(900)])
        .append_query_results([[fuel_entry_row(900, 42, 10)]])
        .into_connection();
    let saved = run_with_user(Some(owner(42)), fuel_entry_use_case(&db).create(new_fuel_entry(Some(42))))
        .await
        .expect("a valid fuel entry is accepted");
    assert_eq!(saved.id, Some(900));
    assert_eq!(saved.origin, FuelEntryOrigin::Manual);
}

// ------------------------------------------------------- EPIC-FU-07-S01 internal tank
use business::domain::internal_tank::InternalTank;
use business::gateway::internal_tank_gateway::InternalTankGateway;
use business::use_cases::internal_tank_use_case::{
    ALERT_LEVEL_MUST_NOT_BE_NEGATIVE, CAPACITY_MUST_BE_POSITIVE, INTERNAL_TANK_NOT_CONFIGURED,
    InternalTankUseCase, REFERENCE_STOCK_OUT_OF_RANGE, RESERVE_LEVEL_MUST_NOT_BE_NEGATIVE,
};

fn internal_tank_use_case(db: &DatabaseConnection) -> InternalTankUseCase {
    InternalTankUseCase::new(InternalTankGateway::new(db.clone()), business::gateway::fuel_entry_gateway::FuelEntryGateway::new(db.clone()))
}

fn new_internal_tank(tenant_id: Option<i64>) -> InternalTank {
    InternalTank {
        id: None,
        uuid: None,
        tenant_id,
        capacity_liters: 10_000.0,
        reference_stock_liters: 6_000.0,
        reference_at: at().naive_utc(),
        alert_level_liters: 4_000.0,
        reserve_level_liters: 500.0,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn internal_tank_row(id: i64, tenant_id: i64) -> entity::internal_tank_entity::Model {
    entity::internal_tank_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(tenant_id),
        capacity_liters: 10_000.0,
        reference_stock_liters: 6_000.0,
        reference_at: at(),
        alert_level_liters: 4_000.0,
        reserve_level_liters: 500.0,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

#[tokio::test]
async fn a_non_positive_tank_capacity_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let tank = InternalTank {
        capacity_liters: 0.0,
        ..new_internal_tank(Some(42))
    };
    let err = run_with_user(Some(owner(42)), internal_tank_use_case(&db).configure(tank))
        .await
        .expect_err("a non-positive capacity is refused");
    assert_eq!(err.message, CAPACITY_MUST_BE_POSITIVE);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn a_reference_stock_above_capacity_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let tank = InternalTank {
        reference_stock_liters: 20_000.0,
        ..new_internal_tank(Some(42))
    };
    let err = run_with_user(Some(owner(42)), internal_tank_use_case(&db).configure(tank))
        .await
        .expect_err("a reference stock above capacity is refused");
    assert_eq!(err.message, REFERENCE_STOCK_OUT_OF_RANGE);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn a_negative_alert_level_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let tank = InternalTank {
        alert_level_liters: -1.0,
        ..new_internal_tank(Some(42))
    };
    let err = run_with_user(Some(owner(42)), internal_tank_use_case(&db).configure(tank))
        .await
        .expect_err("a negative alert level is refused");
    assert_eq!(err.message, ALERT_LEVEL_MUST_NOT_BE_NEGATIVE);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn a_negative_reserve_level_is_rejected_before_any_query() {
    let db = mock().into_connection();
    let tank = InternalTank {
        reserve_level_liters: -1.0,
        ..new_internal_tank(Some(42))
    };
    let err = run_with_user(Some(owner(42)), internal_tank_use_case(&db).configure(tank))
        .await
        .expect_err("a negative reserve level is refused");
    assert_eq!(err.message, RESERVE_LEVEL_MUST_NOT_BE_NEGATIVE);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn configuring_for_the_first_time_inserts_a_new_row() {
    let db = mock()
        .append_query_results([Vec::<entity::internal_tank_entity::Model>::new()]) // no existing row
        .append_exec_results([inserted(900)])
        .append_query_results([[internal_tank_row(900, 42)]])
        .into_connection();
    let saved = run_with_user(
        Some(owner(42)),
        internal_tank_use_case(&db).configure(new_internal_tank(Some(42))),
    )
    .await
    .expect("a valid first configuration is accepted");
    assert_eq!(saved.id, Some(900));
}

/// `TRM-1540`: reconfiguring replaces the manager's last reading in place --
/// it does not append a second row for the same tenant.
#[tokio::test]
async fn reconfiguring_updates_the_existing_row_in_place() {
    let db = mock()
        .append_query_results([[internal_tank_row(900, 42)]]) // existing row found
        .append_exec_results([inserted(900)]) // update, same id
        .append_query_results([[internal_tank_row(900, 42)]]) // refetch
        .into_connection();
    let saved = run_with_user(
        Some(owner(42)),
        internal_tank_use_case(&db).configure(new_internal_tank(Some(42))),
    )
    .await
    .expect("reconfiguring an existing tank is accepted");
    assert_eq!(saved.id, Some(900));
    assert!(!format!("{:?}", log(db)).contains("INSERT"), "reconfiguring must update, not insert");
}

#[tokio::test]
async fn reading_an_unconfigured_tank_is_reported_distinctly() {
    let db = mock()
        .append_query_results([Vec::<entity::internal_tank_entity::Model>::new()])
        .into_connection();
    let err = run_with_user(Some(owner(42)), internal_tank_use_case(&db).get_current(Some(42)))
        .await
        .expect_err("an unconfigured tank is reported, not defaulted");
    assert_eq!(err.message, INTERNAL_TANK_NOT_CONFIGURED);
}

// ------------------------------------------------------- EPIC-FU-02-S01 fuel provider sync
use business::domain::business_error::BusinessError;
use business::gateway::fuel_provider::{AckStatus, FuelProvider, ProviderFuelTransaction};
use business::use_cases::fuel_sync_use_case::FuelSyncUseCase;
use std::sync::Mutex;

struct FakeFuelProvider {
    transactions: Vec<ProviderFuelTransaction>,
    acked: Mutex<Vec<(String, AckStatus)>>,
}

impl FakeFuelProvider {
    fn new(transactions: Vec<ProviderFuelTransaction>) -> Self {
        Self {
            transactions,
            acked: Mutex::new(Vec::new()),
        }
    }
}

impl FuelProvider for FakeFuelProvider {
    async fn fetch_pending(&self) -> Result<Vec<ProviderFuelTransaction>, BusinessError> {
        Ok(self.transactions.clone())
    }

    async fn acknowledge(&self, results: &[(String, AckStatus)]) -> Result<(), BusinessError> {
        *self.acked.lock().unwrap() = results.to_vec();
        Ok(())
    }
}

fn fuel_sync_use_case(db: &DatabaseConnection, provider: FakeFuelProvider) -> FuelSyncUseCase<FakeFuelProvider> {
    FuelSyncUseCase::new(provider, FuelEntryGateway::new(db.clone()), VehicleGateway::new(db.clone()), business::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway::new(db.clone()))
}

fn provider_transaction(external_id: &str, fleet_prefix: Option<&str>) -> ProviderFuelTransaction {
    ProviderFuelTransaction {
        external_id: external_id.to_string(),
        fleet_prefix: fleet_prefix.map(str::to_string),
        plate: None,
        fleet_name: None,
        recorded_at: Some(at()),
        volume_liters: 50.0,
        value_cents: 30_000,
        odometer_km: Some(123_456.0),
        station: Some("Posto Ipiranga".into()),
        full_tank: true,
    }
}

/// `TRM-508`: a transaction whose vehicle cannot be resolved is acknowledged
/// `PENDENTE`, not silently dropped or imported anyway.
#[tokio::test]
async fn a_transaction_with_no_matching_vehicle_is_reported_pending() {
    let provider = FakeFuelProvider::new(vec![provider_transaction("ext-1", Some("999"))]);
    let db = mock()
        .append_query_results([Vec::<entity::tenant_rule_setting_entity::Model>::new()]) // default thresholds
        .append_query_results([Vec::<entity::vehicle_entity::Model>::new()]) // find_by_prefix: none
        .into_connection();

    let outcome = run_with_user(Some(owner(42)), fuel_sync_use_case(&db, provider).sync(Some(42)))
        .await
        .expect("a sync with an unmatched transaction still succeeds overall");
    assert_eq!(outcome.fetched, 1);
    assert_eq!(outcome.unmatched, 1);
    assert_eq!(outcome.imported, 0);
    assert!(!format!("{:?}", log(db)).contains("INSERT"), "an unmatched transaction is never written");
}

/// `TRM-511`/`514`: a transaction naming an unseen `externo_id` is imported
/// as a new ledger row, `origin` stamped `CtaSync`.
#[tokio::test]
async fn a_new_transaction_for_a_matched_vehicle_is_imported() {
    let provider = FakeFuelProvider::new(vec![provider_transaction("ext-2", Some("2642"))]);
    let db = mock()
        .append_query_results([Vec::<entity::tenant_rule_setting_entity::Model>::new()]) // default thresholds
        .append_query_results([[entity::vehicle_entity::Model {
            prefix: Some("2642".into()),
            ..vehicle_row(10, 42, "ABC1D23", "Active")
        }]]) // find_by_prefix: matched
        .append_query_results([Vec::<entity::fuel_entry_entity::Model>::new()]) // find_by_provider_transaction_id: none
        .append_query_results([Vec::<entity::fuel_entry_entity::Model>::new()]) // no driver record to reconcile with
        .append_exec_results([inserted(900)])
        .append_query_results([[fuel_entry_row(900, 42, 10)]])
        .into_connection();

    let outcome = run_with_user(Some(owner(42)), fuel_sync_use_case(&db, provider).sync(Some(42)))
        .await
        .expect("a valid sync succeeds");
    assert_eq!(outcome.imported, 1);
    assert_eq!(outcome.unmatched, 0);
}

/// `TRM-512`: a re-sent `externo_id` with a changed field is corrected in
/// place, never re-imported as a second row.
#[tokio::test]
async fn a_resent_transaction_with_a_changed_field_is_corrected_in_place() {
    let mut tx = provider_transaction("ext-3", Some("2642"));
    tx.volume_liters = 99.0; // differs from fuel_entry_row's own 50.0
    let provider = FakeFuelProvider::new(vec![tx]);
    let db = mock()
        .append_query_results([Vec::<entity::tenant_rule_setting_entity::Model>::new()]) // default thresholds
        .append_query_results([[entity::vehicle_entity::Model {
            prefix: Some("2642".into()),
            ..vehicle_row(10, 42, "ABC1D23", "Active")
        }]])
        .append_query_results([[fuel_entry_row(900, 42, 10)]]) // existing row found
        .append_exec_results([inserted(900)]) // corrected in place, same id
        .append_query_results([[fuel_entry_row(900, 42, 10)]])
        .into_connection();

    let outcome = run_with_user(Some(owner(42)), fuel_sync_use_case(&db, provider).sync(Some(42)))
        .await
        .expect("a correction succeeds");
    assert_eq!(outcome.updated, 1);
    assert_eq!(outcome.imported, 0);
}

/// `TRM-512`'s own qualifier: "only if one of them actually changed" -- a
/// re-sent transaction identical to what is already stored writes nothing.
#[tokio::test]
async fn a_resent_transaction_with_no_actual_change_writes_nothing() {
    let provider = FakeFuelProvider::new(vec![provider_transaction("ext-4", Some("2642"))]);
    let db = mock()
        .append_query_results([Vec::<entity::tenant_rule_setting_entity::Model>::new()]) // default thresholds
        .append_query_results([[entity::vehicle_entity::Model {
            prefix: Some("2642".into()),
            ..vehicle_row(10, 42, "ABC1D23", "Active")
        }]])
        .append_query_results([[fuel_entry_row(900, 42, 10)]]) // identical to the incoming transaction
        .into_connection();

    let outcome = run_with_user(Some(owner(42)), fuel_sync_use_case(&db, provider).sync(Some(42)))
        .await
        .expect("a no-op correction still succeeds");
    assert_eq!(outcome.updated, 0);
    assert_eq!(outcome.imported, 0);
    assert!(!format!("{:?}", log(db)).contains("UPDATE"), "nothing changed, so nothing is written");
}

// ------------------------------------------------------- EPIC-MT-07-S02 preventive work order
use business::gateway::preventive_plan_gateway::PreventivePlanGateway;
use business::use_cases::preventive_plan_use_case::{PLAN_NOT_DUE, PreventivePlanUseCase};

fn preventive_plan_use_case(db: &DatabaseConnection) -> PreventivePlanUseCase {
    PreventivePlanUseCase::new(
        PreventivePlanGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        business::gateway::work_order_gateway::WorkOrderGateway::new(db.clone()),
        business::gateway::work_order_item_gateway::WorkOrderItemGateway::new(db.clone()),
        business::gateway::preventive_plan_extension_gateway::PreventivePlanExtensionGateway::new(db.clone()),
        business::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway::new(db.clone()),
    )
}

fn preventive_plan_row() -> entity::preventive_plan_entity::Model {
    entity::preventive_plan_entity::Model {
        id: 7,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        vehicle_id: 10,
        plan_name: "Oil".into(),
        control_type: "Kilometers".into(),
        interval_km: Some(10_000.0),
        interval_days: None,
        last_service_km: Some(50_000.0),
        last_service_date: None,
        extension_limit_km: None,
        last_work_order_id: None,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

fn vehicle_at_km(km: f64) -> vehicle_entity::Model {
    vehicle_entity::Model {
        odometer_km: Some(km),
        ..vehicle_row(10, 42, "ABC1D23", "Active")
    }
}

#[tokio::test]
async fn a_plan_that_is_not_due_opens_no_work_order() {
    let db = mock()
        .append_query_results([[preventive_plan_row()]])
        .append_query_results([[vehicle_at_km(51_000.0)]])
        .append_query_results([Vec::<entity::tenant_rule_setting_entity::Model>::new()]) // default thresholds
        .into_connection();
    let err = run_with_user(Some(owner(42)), preventive_plan_use_case(&db).generate_work_order(UUID.into()))
        .await
        .expect_err("a plan that is not due generates nothing");
    assert_eq!(err.message, PLAN_NOT_DUE);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

#[tokio::test]
async fn a_due_plan_with_an_open_work_order_returns_it_instead_of_a_duplicate() {
    let db = mock()
        .append_query_results([[preventive_plan_row()]])
        .append_query_results([[vehicle_at_km(60_000.0)]])
        .append_query_results([Vec::<entity::tenant_rule_setting_entity::Model>::new()]) // default thresholds
        .append_query_results([[work_order_row(97, 42, 10)]])
        .into_connection();
    let (work_order, created) =
        run_with_user(Some(owner(42)), preventive_plan_use_case(&db).generate_work_order(UUID.into()))
            .await
            .expect("an existing open order is returned");
    assert!(!created);
    assert_eq!(work_order.id, Some(97));
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

#[tokio::test]
async fn a_due_plan_with_no_open_work_order_opens_a_preventive_one() {
    let db = mock()
        .append_query_results([[preventive_plan_row()]])
        .append_query_results([[vehicle_at_km(60_000.0)]])
        .append_query_results([Vec::<entity::tenant_rule_setting_entity::Model>::new()]) // default thresholds
        .append_query_results([Vec::<entity::work_order_entity::Model>::new()])
        .append_query_results([[vehicle_at_km(60_000.0)]])
        .append_exec_results([inserted(98)])
        .append_query_results([[work_order_row(98, 42, 10)]])
        .into_connection();
    let (work_order, created) =
        run_with_user(Some(owner(42)), preventive_plan_use_case(&db).generate_work_order(UUID.into()))
            .await
            .expect("a due plan opens a work order");
    assert!(created);
    assert_eq!(work_order.id, Some(98));
    let sql = format!("{:?}", log(db));
    assert!(sql.contains("INSERT") && sql.contains("Preventive"), "{sql}");
}

#[tokio::test]
async fn concluding_a_preventive_work_order_renews_its_plan_and_cancels_the_others() {
    let preventive = |id: i64, status: &str| entity::work_order_entity::Model {
        origin: "Preventive".into(),
        preventive_plan_id: Some(7),
        status: status.into(),
        ..work_order_row(id, 42, 10)
    };
    let db = mock()
        .append_query_results([[preventive(97, "Open")]]) // find the work order
        .append_query_results([Vec::<entity::work_order_item_entity::Model>::new()]) // no items: all settled
        .append_exec_results([inserted(97)]) // conclude
        .append_query_results([[preventive(97, "Concluded")]])
        .append_query_results([[vehicle_at_km(61_000.0)]]) // current odometer
        .append_query_results([[preventive_plan_row()]]) // the plan
        .append_exec_results([inserted(7)]) // renew the plan
        .append_query_results([[preventive_plan_row()]])
        .append_query_results([[preventive(99, "Open")]]) // another still-open order for the plan
        .append_query_results([[preventive(99, "Open")]]) // cancel: find it
        .append_exec_results([inserted(99)]) // cancel
        .append_query_results([[preventive(99, "Cancelled")]])
        .into_connection();
    let concluded = run_with_user(Some(owner(42)), work_order_use_case(&db).conclude(97))
        .await
        .expect("conclude succeeds");
    assert_eq!(concluded.status, business::domain::enums::WorkOrderStatus::Concluded);
    let sql = format!("{:?}", log(db));
    assert!(sql.contains("UPDATE `preventive_plan`") && sql.contains("61000"), "{sql}");
    assert!(sql.contains("Cancelled"), "{sql}");
}

#[tokio::test]
async fn a_resolved_item_linked_to_a_plan_renews_that_plan_when_the_order_is_concluded() {
    let order = |status: &str| entity::work_order_entity::Model {
        status: status.into(),
        ..work_order_row(97, 42, 10)
    };
    let linked_item = entity::work_order_item_entity::Model {
        preventive_plan_id: Some(7),
        ..work_order_item_row(5, 42, 97, "Resolved")
    };
    let db = mock()
        .append_query_results([[order("Open")]])
        .append_query_results([[linked_item]]) // the one resolved item, linked to plan 7
        .append_exec_results([inserted(97)])
        .append_query_results([[order("Concluded")]])
        .append_query_results([Vec::<entity::preventive_plan_extension_entity::Model>::new()]) // not an extension item
        .append_query_results([[vehicle_at_km(61_000.0)]])
        .append_query_results([[preventive_plan_row()]])
        .append_exec_results([inserted(7)])
        .append_query_results([[preventive_plan_row()]])
        .append_query_results([Vec::<entity::work_order_entity::Model>::new()]) // nothing else open
        .into_connection();
    run_with_user(Some(owner(42)), work_order_use_case(&db).conclude(97))
        .await
        .expect("conclude succeeds");
    let sql = format!("{:?}", log(db));
    assert!(sql.contains("UPDATE `preventive_plan`") && sql.contains("61000"), "{sql}");
}

#[tokio::test]
async fn an_item_cannot_name_another_vehicles_preventive_plan() {
    let other_vehicle_plan = entity::preventive_plan_entity::Model {
        vehicle_id: 11,
        ..preventive_plan_row()
    };
    let db = mock()
        .append_query_results([[work_order_row(96, 42, 10)]])
        .append_query_results([[other_vehicle_plan]])
        .into_connection();
    let item = WorkOrderItem {
        preventive_plan_id: Some(7),
        ..new_work_order_item(WorkOrderItemStatus::Pending)
    };
    let err = run_with_user(Some(owner(42)), work_order_use_case(&db).add_item(96, item))
        .await
        .expect_err("a plan of another vehicle is refused");
    assert_eq!(err.message, business::use_cases::work_order_use_case::PLAN_NOT_ON_THIS_VEHICLE);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

fn extension_row(item_id: i64) -> entity::preventive_plan_extension_entity::Model {
    entity::preventive_plan_extension_entity::Model {
        id: 3,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        preventive_plan_id: 7,
        work_order_item_id: item_id,
        inspection_km: 58_000.0,
        granted_km: 5_000.0,
        resulting_limit_km: 63_000.0,
        description: "Oil still clean".into(),
        previous_last_service_km: Some(50_000.0),
        previous_last_service_date: None,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

fn linked_pending_item() -> entity::work_order_item_entity::Model {
    entity::work_order_item_entity::Model {
        preventive_plan_id: Some(7),
        ..work_order_item_row(5, 42, 97, "Pending")
    }
}

#[tokio::test]
async fn an_extension_resolves_the_item_records_an_entry_and_leaves_the_order_partially_resolved() {
    let db = mock()
        .append_query_results([[preventive_plan_row()]]) // plan (find_by_uuid)
        .append_query_results([[vehicle_at_km(58_000.0)]]) // status of the plan
        .append_query_results([Vec::<entity::tenant_rule_setting_entity::Model>::new()]) // default thresholds
        .append_query_results([Vec::<entity::tenant_rule_setting_entity::Model>::new()]) // default thresholds
        .append_query_results([[linked_pending_item()]]) // item by uuid
        .append_query_results([[work_order_row(97, 42, 10)]]) // its order
        .append_query_results([[vehicle_at_km(58_000.0)]]) // default inspection km
        .append_exec_results([inserted(3)]) // audit entry
        .append_query_results([[extension_row(5)]])
        .append_exec_results([inserted(7)]) // plan limit
        .append_query_results([[preventive_plan_row()]])
        .append_exec_results([inserted(5)]) // item resolved
        .append_query_results([[work_order_item_row(5, 42, 97, "Resolved")]])
        .append_exec_results([inserted(97)]) // order partially resolved
        .append_query_results([[work_order_row(97, 42, 10)]])
        .into_connection();
    let entry = run_with_user(
        Some(owner(42)),
        preventive_plan_use_case(&db).extend(UUID.into(), UUID.into(), None, 5_000.0, "Oil still clean".into()),
    )
    .await
    .expect("extension succeeds");
    assert_eq!(entry.resulting_limit_km, 63_000.0);
    let sql = format!("{:?}", log(db));
    assert!(sql.contains("preventive_plan_extension") && sql.contains("PartiallyResolved"), "{sql}");
    assert!(sql.contains("Resolved") && sql.contains("63000"), "{sql}");
}

#[tokio::test]
async fn an_extension_is_refused_for_an_item_not_linked_to_the_plan() {
    let unlinked = entity::work_order_item_entity::Model {
        preventive_plan_id: None,
        ..work_order_item_row(5, 42, 97, "Pending")
    };
    let db = mock()
        .append_query_results([[preventive_plan_row()]])
        .append_query_results([[vehicle_at_km(58_000.0)]])
        .append_query_results([Vec::<entity::tenant_rule_setting_entity::Model>::new()]) // default thresholds
        .append_query_results([Vec::<entity::tenant_rule_setting_entity::Model>::new()]) // default thresholds
        .append_query_results([[unlinked]])
        .append_query_results([[work_order_row(97, 42, 10)]])
        .into_connection();
    let err = run_with_user(
        Some(owner(42)),
        preventive_plan_use_case(&db).extend(UUID.into(), UUID.into(), None, 5_000.0, "x".into()),
    )
    .await
    .expect_err("unlinked item refused");
    assert_eq!(err.message, business::use_cases::preventive_plan_use_case::EXTENSION_ITEM_NOT_LINKED);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

#[tokio::test]
async fn an_extension_grant_above_the_maximum_is_refused_before_any_write() {
    let db = mock()
        .append_query_results([[preventive_plan_row()]])
        .append_query_results([[vehicle_at_km(58_000.0)]])
        .append_query_results([Vec::<entity::tenant_rule_setting_entity::Model>::new()]) // default thresholds
        .append_query_results([Vec::<entity::tenant_rule_setting_entity::Model>::new()]) // default thresholds
        .into_connection();
    let err = run_with_user(
        Some(owner(42)),
        preventive_plan_use_case(&db).extend(UUID.into(), UUID.into(), None, 50_001.0, "x".into()),
    )
    .await
    .expect_err("over the maximum");
    assert_eq!(err.message, business::use_cases::preventive_plan_use_case::EXTENSION_GRANT_INVALID);
}

#[tokio::test]
async fn concluding_an_order_whose_item_was_resolved_by_extension_does_not_renew_the_plan() {
    let order = |status: &str| entity::work_order_entity::Model {
        status: status.into(),
        preventive_plan_id: Some(7),
        ..work_order_row(97, 42, 10)
    };
    let extended_item = entity::work_order_item_entity::Model {
        preventive_plan_id: Some(7),
        ..work_order_item_row(5, 42, 97, "Resolved")
    };
    let db = mock()
        .append_query_results([[order("PartiallyResolved")]])
        .append_query_results([[extended_item]])
        .append_exec_results([inserted(97)])
        .append_query_results([[order("Concluded")]])
        .append_query_results([[extension_row(5)]]) // this item resolved by extension
        .into_connection();
    run_with_user(Some(owner(42)), work_order_use_case(&db).conclude(97))
        .await
        .expect("conclude succeeds");
    assert!(!format!("{:?}", log(db)).contains("UPDATE `preventive_plan`"));
}

fn edited_plan(last_service_km: Option<f64>) -> business::domain::preventive_plan::PreventivePlan {
    use business::commons::entity_mapper::EntityMapper;
    business::domain::preventive_plan::PreventivePlanEntityMapper::from_model(
        entity::preventive_plan_entity::Model {
            last_service_km,
            ..preventive_plan_row()
        },
    )
}

#[tokio::test]
async fn editing_a_plans_base_ends_its_active_extension_but_editing_an_interval_keeps_it() {
    let extended = || entity::preventive_plan_entity::Model {
        extension_limit_km: Some(65_000.0),
        ..preventive_plan_row()
    };
    for (edited, expect_cleared) in [
        (edited_plan(Some(60_000.0)), true), // base moved: new cycle
        (business::domain::preventive_plan::PreventivePlan { interval_km: Some(8_000.0), ..edited_plan(Some(50_000.0)) }, false),
    ] {
        let db = mock()
            .append_query_results([[extended()]]) // current plan
            .append_query_results([[vehicle_at_km(58_000.0)]]) // its status
        .append_query_results([Vec::<entity::tenant_rule_setting_entity::Model>::new()]) // default thresholds
            .append_exec_results([inserted(7)])
            .append_query_results([[preventive_plan_row()]])
            .into_connection();
        run_with_user(Some(owner(42)), preventive_plan_use_case(&db).update(business::domain::preventive_plan::PreventivePlan {
            uuid: Some(UUID.into()),
            ..edited
        }))
        .await
        .expect("update succeeds");
        let sql = format!("{:?}", log(db));
        assert_eq!(sql.contains("65000"), !expect_cleared, "{sql}");
    }
}

#[tokio::test]
async fn editing_a_plan_still_requires_the_interval_its_control_type_needs() {
    let db = mock()
        .append_query_results([[preventive_plan_row()]])
        .append_query_results([[vehicle_at_km(58_000.0)]])
        .append_query_results([Vec::<entity::tenant_rule_setting_entity::Model>::new()]) // default thresholds
        .into_connection();
    let bad = business::domain::preventive_plan::PreventivePlan {
        uuid: Some(UUID.into()),
        interval_km: None,
        ..edited_plan(Some(50_000.0))
    };
    let err = run_with_user(Some(owner(42)), preventive_plan_use_case(&db).update(bad))
        .await
        .expect_err("missing interval");
    assert_eq!(err.message, business::use_cases::preventive_plan_use_case::INTERVAL_REQUIRED);
}

#[tokio::test]
async fn a_new_cycle_may_name_its_originating_order_only_if_it_is_on_the_plans_vehicle() {
    let edited = |order_id| business::domain::preventive_plan::PreventivePlan {
        uuid: Some(UUID.into()),
        last_work_order_id: Some(order_id),
        ..edited_plan(Some(60_000.0)) // base moved: new cycle
    };
    let db = mock()
        .append_query_results([[preventive_plan_row()]])
        .append_query_results([[vehicle_at_km(58_000.0)]])
        .append_query_results([Vec::<entity::tenant_rule_setting_entity::Model>::new()]) // default thresholds
        .append_query_results([[work_order_row(97, 42, 11)]]) // another vehicle's order
        .into_connection();
    let err = run_with_user(Some(owner(42)), preventive_plan_use_case(&db).update(edited(97)))
        .await
        .expect_err("another vehicle's order refused");
    assert_eq!(err.message, business::use_cases::preventive_plan_use_case::ORIGIN_ORDER_NOT_ON_VEHICLE);

    let db = mock()
        .append_query_results([[preventive_plan_row()]])
        .append_query_results([[vehicle_at_km(58_000.0)]])
        .append_query_results([Vec::<entity::tenant_rule_setting_entity::Model>::new()]) // default thresholds
        .append_query_results([[work_order_row(97, 42, 10)]])
        .append_exec_results([inserted(7)])
        .append_query_results([[entity::preventive_plan_entity::Model {
            last_work_order_id: Some(97),
            ..preventive_plan_row()
        }]])
        .into_connection();
    let saved = run_with_user(Some(owner(42)), preventive_plan_use_case(&db).update(edited(97)))
        .await
        .expect("a same-vehicle order is accepted");
    assert_eq!(saved.last_work_order_id, Some(97));
}

// ------------------------------------------------------- EPIC-MT-07-S09 technical inspection
use business::domain::technical_inspection::TechnicalInspection;
use business::domain::technical_inspection_item::TechnicalInspectionItem;
use business::use_cases::technical_inspection_use_case::TechnicalInspectionUseCase;

fn technical_inspection_use_case(db: &DatabaseConnection) -> TechnicalInspectionUseCase {
    use business::gateway::*;
    TechnicalInspectionUseCase::new(
        technical_inspection_gateway::TechnicalInspectionGateway::new(db.clone()),
        technical_inspection_item_gateway::TechnicalInspectionItemGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        work_order_gateway::WorkOrderGateway::new(db.clone()),
        work_order_item_gateway::WorkOrderItemGateway::new(db.clone()),
        PreventivePlanGateway::new(db.clone()),
        preventive_plan_alert_gateway::PreventivePlanAlertGateway::new(db.clone()),
        inspection_model_gateway::InspectionModelGateway::new(db.clone()),
    )
}

fn inspection_row() -> entity::technical_inspection_entity::Model {
    entity::technical_inspection_entity::Model {
        id: 5,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        vehicle_id: 10,
        inspection_model: "Brake check".into(),
        inspected_at: NaiveDate::from_ymd_opt(2026, 9, 30).unwrap(),
        odometer_km: 1_000.0,
        observation: None,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

fn inspection_item_row(id: i64, description: &str) -> entity::technical_inspection_item_entity::Model {
    entity::technical_inspection_item_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        technical_inspection_id: 5,
        description: description.into(),
        conforming: false,
        observation: None,
        work_order_item_id: None,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

fn new_inspection() -> TechnicalInspection {
    TechnicalInspection {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        vehicle_id: 10,
        inspection_model: "Brake check".into(),
        inspected_at: NaiveDate::from_ymd_opt(2026, 9, 30).unwrap(),
        odometer_km: 0.0,
        observation: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn nonconforming(description: &str) -> TechnicalInspectionItem {
    TechnicalInspectionItem {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        technical_inspection_id: 0,
        description: description.into(),
        conforming: false,
        observation: Some("seen again".into()),
        work_order_item_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn a_non_conformity_matching_a_pending_item_of_an_open_order_opens_no_new_order() {
    let pending = entity::work_order_item_entity::Model {
        description: "Troca do óleo".into(),
        ..work_order_item_row(60, 42, 97, "Pending")
    };
    let db = mock()
        .append_query_results([[vehicle_at_km(1_000.0)]])
        .append_query_results([Vec::<entity::inspection_model_entity::Model>::new()]) // unknown model
        .append_query_results([[inspection_row()]]) // read back after insert
        .append_query_results([[work_order_row(97, 42, 10)]]) // the vehicle's open orders
        .append_query_results([[pending.clone()]]) // that order's items
        .append_query_results([[pending]]) // read back after the note update
        .append_query_results([[inspection_item_row(1, "troca oleo")]])
        .append_query_results([Vec::<entity::preventive_plan_entity::Model>::new()])
        .append_exec_results([inserted(5), inserted(60), inserted(1)])
        .into_connection();
    let (_, items, opened) = run_with_user(
        Some(owner(42)),
        technical_inspection_use_case(&db).submit(new_inspection(), vec![nonconforming("troca oleo")]),
    )
    .await
    .expect("the inspection is recorded");
    assert!(!opened, "the defect is already on an open order");
    assert_eq!(items.len(), 1);
    let sql = format!("{:?}", log(db));
    assert!(!sql.contains("INSERT INTO `work_order`"), "{sql}");
    assert!(sql.contains("UPDATE `work_order_item`"), "{sql}");
}

#[tokio::test]
async fn an_unmatched_non_conformity_opens_one_inspection_order() {
    let pending = entity::work_order_item_entity::Model {
        description: "Troca do óleo".into(),
        ..work_order_item_row(60, 42, 97, "Pending")
    };
    let db = mock()
        .append_query_results([[vehicle_at_km(1_000.0)]])
        .append_query_results([Vec::<entity::inspection_model_entity::Model>::new()])
        .append_query_results([[inspection_row()]])
        .append_query_results([[work_order_row(97, 42, 10)]])
        .append_query_results([[pending]])
        .append_query_results([[work_order_row(98, 42, 10)]]) // the new order read back
        .append_query_results([[work_order_item_row(61, 42, 98, "Pending")]])
        .append_query_results([[work_order_item_row(62, 42, 98, "Pending")]])
        .append_query_results([[inspection_item_row(1, "Freio dianteiro")]])
        .append_query_results([[inspection_item_row(2, "Freio traseiro")]])
        .append_query_results([Vec::<entity::preventive_plan_entity::Model>::new()])
        .append_exec_results([inserted(5), inserted(98), inserted(61), inserted(62), inserted(1), inserted(2)])
        .into_connection();
    let (_, _, opened) = run_with_user(
        Some(owner(42)),
        technical_inspection_use_case(&db)
            .submit(new_inspection(), vec![nonconforming("Freio dianteiro"), nonconforming("Freio traseiro")]),
    )
    .await
    .expect("the inspection is recorded");
    assert!(opened);
    let sql = format!("{:?}", log(db));
    assert_eq!(sql.matches("INSERT INTO `work_order` ").count(), 1, "one order, not one per item: {sql}");
    assert!(sql.contains("Inspection"), "{sql}");
}

#[tokio::test]
async fn a_model_that_generates_no_work_order_opens_and_links_nothing() {
    let model = entity::inspection_model_entity::Model {
        id: 3,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        name: "Brake check".into(),
        generates_work_order: false,
        periodicity_days: None,
        observation: None,
        active: true,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    };
    let db = mock()
        .append_query_results([[vehicle_at_km(1_000.0)]])
        .append_query_results([[model]])
        .append_query_results([[inspection_row()]])
        .append_query_results([[inspection_item_row(1, "Freio dianteiro")]])
        .append_query_results([Vec::<entity::preventive_plan_entity::Model>::new()])
        .append_exec_results([inserted(5), inserted(1)])
        .into_connection();
    let (_, _, opened) = run_with_user(
        Some(owner(42)),
        technical_inspection_use_case(&db).submit(new_inspection(), vec![nonconforming("Freio dianteiro")]),
    )
    .await
    .expect("the inspection is recorded");
    assert!(!opened);
    let sql = format!("{:?}", log(db));
    assert!(!sql.contains("`work_order`"), "no order is read or written: {sql}");
}

#[tokio::test]
async fn a_configured_tank_with_no_fuelling_record_at_all_shows_no_stock() {
    let db = mock()
        .append_query_results([[internal_tank_row(1, 42)]])
        .append_query_results(count(0))
        .append_query_results([Vec::<entity::fuel_entry_entity::Model>::new()])
        .into_connection();
    let (_, stock) = run_with_user(Some(owner(42)), internal_tank_use_case(&db).stock(Some(42)))
        .await
        .expect("the tank exists");
    assert_eq!(stock.current_stock_liters, None, "no full tank computed from an empty list");
    assert!(!stock.delivery_alert);
    assert!(stock.suppressed.is_some());
}

// ------------------------------------------------------- EPIC-FU-03-S01 driver receipt
use business::use_cases::fuel_receipt_use_case::{FuelReceipt, FuelReceiptUseCase, ODOMETER_OUTSIDE_WINDOW, PREFIX_MISMATCH};

fn fuel_receipt_use_case(db: &DatabaseConnection) -> FuelReceiptUseCase {
    FuelReceiptUseCase::new(
        business::gateway::fuel_entry_gateway::FuelEntryGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        UserGateway::new(db.clone()),
        business::gateway::km_evolution_gateway::KmEvolutionGateway::new(db.clone()),
        business::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway::new(db.clone()),
    )
}

fn a_receipt(odometer_km: Option<f64>, override_odometer: bool) -> FuelReceipt {
    FuelReceipt {
        tenant_id: Some(42),
        vehicle_id: 10,
        driver_id: 5,
        confirmed_prefix: "2642".into(),
        recorded_at: at().naive_utc(),
        volume_liters: 50.0,
        value_cents: 30_000,
        odometer_km,
        station: None,
        full_tank: true,
        override_odometer,
    }
}

fn receipt_vehicle() -> vehicle_entity::Model {
    vehicle_entity::Model { prefix: Some("2642".into()), ..vehicle_row(10, 42, "ABC1D23", "Active") }
}

fn a_driver() -> entity::user_entity::Model {
    user_row(5, "driver@example.com", "Driver", Some(42), "x")
}

#[tokio::test]
async fn a_receipt_whose_prefix_does_not_match_the_vehicle_is_refused() {
    let db = mock().append_query_results([[a_driver()]]).append_query_results([[receipt_vehicle()]]).into_connection();
    let mut receipt = a_receipt(None, false);
    receipt.confirmed_prefix = "9999".into();
    let err = run_with_user(Some(owner(42)), fuel_receipt_use_case(&db).record(receipt)).await.expect_err("refused");
    assert_eq!(err.message, PREFIX_MISMATCH);
}

#[tokio::test]
async fn a_resubmitted_fuelling_answers_with_the_existing_record() {
    let db = mock()
        .append_query_results([[a_driver()]])
        .append_query_results([[receipt_vehicle()]])
        .append_query_results([Vec::<entity::tenant_rule_setting_entity::Model>::new()]) // default thresholds
        .append_query_results([[fuel_entry_row(900, 42, 10)]]) // 50 l within 10 minutes
        .into_connection();
    let (entry, created) = run_with_user(Some(owner(42)), fuel_receipt_use_case(&db).record(a_receipt(None, false)))
        .await
        .expect("answered");
    assert!(!created);
    assert_eq!(entry.id, Some(900));
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

#[tokio::test]
async fn a_divergent_odometer_is_refused_unless_the_driver_overrides_and_then_it_is_noted() {
    let queue = |db: MockDatabase| {
        db.append_query_results([[a_driver()]])
            .append_query_results([[receipt_vehicle()]])
            .append_query_results([Vec::<entity::tenant_rule_setting_entity::Model>::new()])
            .append_query_results([Vec::<entity::fuel_entry_entity::Model>::new()])
            .append_query_results([[km_evolution_row(1, 42, 10, 1_000.0, "Manual")]]) // before
            .append_query_results([[km_evolution_row(2, 42, 10, 2_000.0, "Manual")]]) // after
    };
    let db = queue(mock()).into_connection();
    let err = run_with_user(Some(owner(42)), fuel_receipt_use_case(&db).record(a_receipt(Some(900.0), false)))
        .await
        .expect_err("refused");
    assert!(err.message.starts_with(ODOMETER_OUTSIDE_WINDOW), "{}", err.message);
    assert!(err.message.contains("[1000, 2000]"), "the bounds are reported: {}", err.message);

    let mut saved = fuel_entry_row(901, 42, 10);
    saved.odometer_override_note = Some("noted".into());
    let db = queue(mock()).append_exec_results([inserted(901)]).append_query_results([[saved]]).into_connection();
    let (_, created) = run_with_user(Some(owner(42)), fuel_receipt_use_case(&db).record(a_receipt(Some(900.0), true)))
        .await
        .expect("recorded with the override");
    assert!(created);
    let sql = format!("{:?}", log(db));
    assert!(sql.contains("DriverPhoto") && sql.contains("overridden by the driver"), "{sql}");
}

/// `TRM-545`/`548`/`549`: a provider transaction that matches a driver-reported
/// fuelling is folded into it, not imported a second time.
#[tokio::test]
async fn a_provider_transaction_matching_a_driver_fuelling_adopts_it_instead_of_duplicating() {
    let provider = FakeFuelProvider::new(vec![provider_transaction("ext-3", Some("2642"))]);
    let driver_entry = entity::fuel_entry_entity::Model {
        origin: "DriverPhoto".into(),
        reported_by_user_id: Some(5),
        station: Some("Posto do motorista".into()),
        volume_liters: 50.4, // within max(1 l, 3 %) of the provider's 50
        ..fuel_entry_row(800, 42, 10)
    };
    let adopted = entity::fuel_entry_entity::Model {
        provider_transaction_id: Some("ext-3".into()),
        ..driver_entry.clone()
    };
    let db = mock()
        .append_query_results([Vec::<entity::tenant_rule_setting_entity::Model>::new()]) // default thresholds
        .append_query_results([[entity::vehicle_entity::Model {
            prefix: Some("2642".into()),
            ..vehicle_row(10, 42, "ABC1D23", "Active")
        }]])
        .append_query_results([Vec::<entity::fuel_entry_entity::Model>::new()]) // not yet known by transaction id
        .append_query_results([[driver_entry.clone()]]) // the candidate
        .append_query_results([[driver_entry]]) // TRM-549 re-read: still unreconciled
        .append_exec_results([inserted(800)])
        .append_query_results([[adopted]])
        .into_connection();
    let outcome = run_with_user(Some(owner(42)), fuel_sync_use_case(&db, provider).sync(Some(42)))
        .await
        .expect("a valid sync succeeds");
    assert_eq!((outcome.reconciled, outcome.imported), (1, 0));
    let sql = format!("{:?}", log(db));
    assert!(!sql.contains("INSERT INTO `fuel_entry`"), "no second row: {sql}");
    assert!(sql.contains("UPDATE `fuel_entry`") && sql.contains("DriverPhoto"), "{sql}");
}

#[test]
fn the_closest_candidate_within_tolerance_is_chosen_and_a_far_volume_is_not() {
    use business::use_cases::fuel_sync_use_case::pick_candidate;
    let at_naive = at().naive_utc();
    let candidate = |id: i64, hours: i64, liters: f64| entity::fuel_entry_entity::Model {
        recorded_at: at() + chrono::Duration::hours(hours),
        volume_liters: liters,
        ..fuel_entry_row(id, 42, 10)
    };
    let picked = pick_candidate(at_naive, 100.0, vec![candidate(1, 30, 100.0), candidate(2, -2, 102.5), candidate(3, 1, 120.0)], &Default::default());
    // 3 % of 100 l is 3 l: #2 fits and is closest among those that fit; #3 is 20 l off.
    assert_eq!(picked.map(|c| c.id), Some(2));
    // Small volumes use the absolute 1 l tolerance.
    assert!(pick_candidate(at_naive, 10.0, vec![candidate(4, 0, 10.9)], &Default::default()).is_some());
    assert!(pick_candidate(at_naive, 10.0, vec![candidate(5, 0, 11.5)], &Default::default()).is_none());
}

// ------------------------------------------------------- EPIC-FU-04-S02 soft delete, restore, unify
fn fuel_ledger_use_case(db: &DatabaseConnection) -> business::use_cases::fuel_entry_use_case::FuelEntryUseCase {
    business::use_cases::fuel_entry_use_case::FuelEntryUseCase::new(
        business::gateway::fuel_entry_gateway::FuelEntryGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        business::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway::new(db.clone()),
    )
}

#[tokio::test]
async fn unifying_two_fuellings_of_one_day_sums_them_and_soft_deletes_the_absorbed_one() {
    let target = entity::fuel_entry_entity::Model { odometer_km: None, full_tank: false, ..fuel_entry_row(1, 42, 10) };
    let source = entity::fuel_entry_entity::Model {
        id: 2,
        volume_liters: 30.0,
        value_cents: 18_000,
        odometer_km: Some(777.0),
        full_tank: true,
        ..fuel_entry_row(2, 42, 10)
    };
    let db = mock()
        .append_query_results([[target.clone()]])
        .append_query_results([[source.clone()]])
        .append_exec_results([inserted(1), inserted(2)])
        .append_query_results([[target.clone()]]) // read-backs after each update
        .append_query_results([[source.clone()]])
        .into_connection();
    let keeper = run_with_user(Some(owner(42)), fuel_ledger_use_case(&db).unify("t".into(), "s".into()))
        .await
        .expect("unified");
    assert_eq!(keeper.id, Some(1));
    let sql = format!("{:?}", log(db));
    assert!(sql.contains("Unified with"), "the keeper is annotated: {sql}");
    assert!(sql.matches("UPDATE `fuel_entry`").count() == 2, "{sql}");
    // The summed figures and the adopted odometer travel in the keeper's update.
    assert!(sql.contains("80.0") && sql.contains("48000") && sql.contains("777.0"), "{sql}");
}

#[tokio::test]
async fn fuellings_of_different_days_cannot_be_unified() {
    let target = fuel_entry_row(1, 42, 10);
    let source = entity::fuel_entry_entity::Model {
        recorded_at: at() + chrono::Duration::days(1),
        ..fuel_entry_row(2, 42, 10)
    };
    let db = mock().append_query_results([[target]]).append_query_results([[source]]).into_connection();
    let err = run_with_user(Some(owner(42)), fuel_ledger_use_case(&db).unify("t".into(), "s".into()))
        .await
        .expect_err("refused");
    assert_eq!(err.message, business::use_cases::fuel_entry_use_case::UNIFY_DIFFERENT_DAYS);
    assert!(!format!("{:?}", log(db)).contains("UPDATE"));
}

#[tokio::test]
async fn deleting_a_fuelling_only_stamps_it_and_nothing_is_removed() {
    let db = mock()
        .append_query_results([[fuel_entry_row(1, 42, 10)]])
        .append_exec_results([inserted(1)])
        .append_query_results([[fuel_entry_row(1, 42, 10)]])
        .into_connection();
    run_with_user(Some(owner(42)), fuel_ledger_use_case(&db).soft_delete("u".into())).await.expect("deleted");
    let sql = format!("{:?}", log(db));
    assert!(sql.contains("UPDATE `fuel_entry`") && !sql.contains("DELETE"), "{sql}");
}

// ------------------------------------------------------- EPIC-FU-06-S03 tenant gauge settings
use business::domain::fuel_gauge_setting::FuelGaugeSetting;
use business::use_cases::fuel_gauge_setting_use_case::{FuelGaugeSettingUseCase, RATIO_INVALID, SUSPECT_MARGIN_INVALID};

fn gauge_setting_use_case(db: &DatabaseConnection) -> FuelGaugeSettingUseCase {
    FuelGaugeSettingUseCase::new(business::gateway::fuel_gauge_setting_gateway::FuelGaugeSettingGateway::new(db.clone()))
}

fn a_setting(suspect: f64, ratio: f64) -> FuelGaugeSetting {
    FuelGaugeSetting {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        suspect_margin_ratio: suspect,
        set_aside_min_expected_liters: 12.0,
        set_aside_ratio: ratio,
        large_fuelling_ratio: 0.4,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn gauge_thresholds_outside_their_sense_are_refused_before_any_write() {
    let db = mock().into_connection();
    let err = run_with_user(Some(owner(42)), gauge_setting_use_case(&db).configure(a_setting(0.9, 0.5))).await.expect_err("refused");
    assert_eq!(err.message, SUSPECT_MARGIN_INVALID);
    let err = run_with_user(Some(owner(42)), gauge_setting_use_case(&db).configure(a_setting(1.05, 1.5))).await.expect_err("refused");
    assert_eq!(err.message, RATIO_INVALID);
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn a_tenant_with_no_settings_row_gets_the_platform_defaults_and_its_own_once_set() {
    let defaults = business::use_cases::fuel_gauge_use_case::GaugeSettings::default();
    let db = mock().append_query_results([Vec::<entity::fuel_gauge_setting_entity::Model>::new()]).into_connection();
    let got = run_with_user(Some(owner(42)), gauge_setting_use_case(&db).current(Some(42))).await.expect("read");
    assert_eq!(got, defaults);

    let row = entity::fuel_gauge_setting_entity::Model {
        id: 1,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        suspect_margin_ratio: 1.2,
        set_aside_min_expected_liters: 12.0,
        set_aside_ratio: 0.3,
        large_fuelling_ratio: 0.4,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    };
    let db = mock().append_query_results([[row]]).into_connection();
    let got = run_with_user(Some(owner(42)), gauge_setting_use_case(&db).current(Some(42))).await.expect("read");
    assert_eq!((got.suspect_margin_ratio, got.set_aside_ratio), (1.2, 0.3));
}

#[test]
fn a_tenants_reconciliation_tolerance_decides_what_counts_as_the_same_fuelling() {
    use business::domain::tenant_rule_setting::RuleSettings;
    use business::use_cases::fuel_sync_use_case::pick_candidate;
    let candidate = entity::fuel_entry_entity::Model { volume_liters: 105.0, ..fuel_entry_row(1, 42, 10) };
    // 5 l off a 100 l posting: outside the default max(1 l, 3 %) ...
    assert!(pick_candidate(at().naive_utc(), 100.0, vec![candidate.clone()], &RuleSettings::default()).is_none());
    // ... inside a tenant's own 6 % tolerance.
    let loose = RuleSettings { reconciliation_volume_tolerance_ratio: 0.06, ..RuleSettings::default() };
    assert!(pick_candidate(at().naive_utc(), 100.0, vec![candidate], &loose).is_some());
}


// ------------------------------------------------------- EPIC-GA-01-S01 garage service catalogue
use business::domain::enums::GarageServiceGroup;
use business::domain::garage_service_model::{GarageServiceApplicability, GarageServiceModel};
use business::use_cases::garage_service_model_use_case::{
    DUPLICATE_NAME, GarageServiceModelUseCase, INVALID_APPLICABILITY, NAME_REQUIRED as GARAGE_NAME_REQUIRED,
};

fn garage_catalogue(db: &DatabaseConnection) -> GarageServiceModelUseCase {
    GarageServiceModelUseCase::new(
        business::gateway::garage_service_model_gateway::GarageServiceModelGateway::new(db.clone()),
        business::gateway::vehicle_gateway::VehicleGateway::new(db.clone()),
    )
}

fn garage_service(name: &str) -> GarageServiceModel {
    GarageServiceModel {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        name: name.into(),
        name_key: String::new(),
        display_order: 1,
        active: true,
        service_group: GarageServiceGroup::External,
        required_for_departure: true,
        governed_by_tank: false,
        applicability: GarageServiceApplicability::All,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn garage_service_row(id: i64, name: &str, key: &str) -> entity::garage_service_model_entity::Model {
    entity::garage_service_model_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        name: name.into(),
        name_key: key.into(),
        display_order: 1,
        active: true,
        service_group: "External".into(),
        required_for_departure: true,
        governed_by_tank: false,
        applicability: "{\"scope\":\"all\"}".into(),
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

#[tokio::test]
async fn a_blank_name_is_refused_and_a_name_equal_after_normalisation_is_a_duplicate() {
    let db = mock().into_connection();
    let err = run_with_user(Some(owner(42)), garage_catalogue(&db).create(garage_service("  . "))).await.expect_err("refused");
    assert_eq!(err.message, GARAGE_NAME_REQUIRED);

    // "Lavagem  Externa" and the stored "lavagem externa" are one name (TRM-433).
    let db = mock().append_query_results([[garage_service_row(1, "lavagem externa", "lavagem externa")]]).into_connection();
    let err = run_with_user(Some(owner(42)), garage_catalogue(&db).create(garage_service("Lavagem  Externa")))
        .await
        .expect_err("duplicate");
    assert_eq!(err.message, DUPLICATE_NAME);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

#[tokio::test]
async fn a_new_service_is_stored_under_its_normalised_key_and_renaming_to_its_own_key_is_not_a_duplicate() {
    let db = mock()
        .append_query_results([Vec::<entity::garage_service_model_entity::Model>::new()])
        .append_exec_results([inserted(1)])
        .append_query_results([[garage_service_row(1, "Higienização WC", "higienizacao wc")]])
        .into_connection();
    run_with_user(Some(owner(42)), garage_catalogue(&db).create(garage_service("Higienização WC"))).await.expect("created");
    assert!(format!("{:?}", log(db)).contains("higienizacao wc"));

    // Editing the service whose key it already is: the lookup finds itself, which is not a clash.
    let db = mock()
        .append_query_results([[garage_service_row(1, "Higienização WC", "higienizacao wc")]]) // by uuid
        .append_query_results([[garage_service_row(1, "Higienização WC", "higienizacao wc")]]) // by key: itself
        .append_exec_results([inserted(1)])
        .append_query_results([[garage_service_row(1, "Higienização wc", "higienizacao wc")]])
        .into_connection();
    let edited = GarageServiceModel { uuid: Some(UUID.into()), ..garage_service("Higienização wc") };
    run_with_user(Some(owner(42)), garage_catalogue(&db).update(edited)).await.expect("renamed to its own key");
}

#[tokio::test]
async fn a_service_applicability_is_persisted_and_returned() {
    let applicability = GarageServiceApplicability::VehicleTypes(vec!["Ônibus".into(), "onibus".into()]);
    let mut persisted = garage_service_row(1, "Higienização WC", "higienizacao wc");
    persisted.applicability = serde_json::to_string(&GarageServiceApplicability::VehicleTypes(vec!["Ônibus".into()]))
        .expect("serializable applicability");
    let db = mock()
        .append_query_results([Vec::<entity::garage_service_model_entity::Model>::new()])
        .append_exec_results([inserted(1)])
        .append_query_results([[persisted]])
        .into_connection();
    let service = GarageServiceModel {
        applicability,
        ..garage_service("Higienização WC")
    };

    let created = run_with_user(Some(owner(42)), garage_catalogue(&db).create(service))
        .await
        .expect("service created");
    assert_eq!(created.applicability, GarageServiceApplicability::VehicleTypes(vec!["Ônibus".into()]));
    assert!(format!("{:?}", log(db)).contains("vehicleTypes"));
}

#[tokio::test]
async fn a_service_cannot_target_a_vehicle_owned_by_another_tenant() {
    let db = mock()
        .append_query_results([Vec::<entity::garage_service_model_entity::Model>::new()])
        .append_query_results([[vehicle_row(10, 43, "XYZ123", "Active")]])
        .into_connection();
    let service = GarageServiceModel {
        applicability: GarageServiceApplicability::Vehicles(vec![UUID.into()]),
        ..garage_service("Lavagem externa")
    };

    let error = run_with_user(Some(owner(42)), garage_catalogue(&db).create(service))
        .await
        .expect_err("cross-tenant vehicle must not enter a service scope");
    assert_eq!(error.message, INVALID_APPLICABILITY);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

#[tokio::test]
async fn an_empty_applicability_list_is_refused() {
    let db = mock()
        .append_query_results([Vec::<entity::garage_service_model_entity::Model>::new()])
        .into_connection();
    let service = GarageServiceModel {
        applicability: GarageServiceApplicability::VehicleTypes(vec![]),
        ..garage_service("Lavagem externa")
    };

    let error = run_with_user(Some(owner(42)), garage_catalogue(&db).create(service))
        .await
        .expect_err("empty applicability must not silently mean all vehicles");
    assert_eq!(error.message, INVALID_APPLICABILITY);
}

// ------------------------------------------------------- EPIC-GA-02 triage and service marking
use business::domain::enums::GarageServiceState;
use business::use_cases::garage_attendance_use_case::{
    ALREADY_ACTIVE, ATTENDANCE_CLOSED, GarageAttendanceUseCase, NO_SERVICES,
};

fn garage_attendance_use_case(db: &DatabaseConnection) -> GarageAttendanceUseCase {
    use business::gateway::*;
    GarageAttendanceUseCase::new(
        garage_attendance_gateway::GarageAttendanceGateway::new(db.clone()),
        garage_service_gateway::GarageServiceGateway::new(db.clone()),
        garage_service_log_gateway::GarageServiceLogGateway::new(db.clone()),
        garage_service_model_gateway::GarageServiceModelGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
    )
}

fn attendance_row(id: i64, status: &str, active: Option<i32>) -> entity::garage_attendance_entity::Model {
    entity::garage_attendance_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        vehicle_id: 10,
        attendance_date: NaiveDate::from_ymd_opt(2026, 9, 30).unwrap(),
        checked_in_at: at(),
        status: status.into(),
        manual_priority: Some(3),
        released_at: None,
        origin: "Manual".into(),
        active_marker: active,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

fn service_record_row(id: i64, key: &str, state: &str) -> entity::garage_service_entity::Model {
    entity::garage_service_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        attendance_id: 1,
        service_model_id: 1,
        name_key: key.into(),
        state: state.into(),
        performed_at: None,
        marked_at: None,
        forced_pending_at: None,
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

#[tokio::test]
async fn a_vehicle_with_an_active_triage_cannot_have_a_second_and_an_empty_catalogue_opens_none() {
    use business::domain::enums::GarageAttendanceOrigin;
    let db = mock()
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([[attendance_row(1, "Open", Some(1))]])
        .into_connection();
    let err = run_with_user(Some(owner(42)), garage_attendance_use_case(&db).open(Some(42), 10, None, GarageAttendanceOrigin::Manual))
        .await
        .expect_err("refused");
    assert_eq!(err.message, ALREADY_ACTIVE);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));

    let db = mock()
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([Vec::<entity::garage_attendance_entity::Model>::new()])
        .append_query_results([Vec::<entity::garage_service_model_entity::Model>::new()])
        .into_connection();
    let err = run_with_user(Some(owner(42)), garage_attendance_use_case(&db).open(Some(42), 10, None, GarageAttendanceOrigin::Manual))
        .await
        .expect_err("refused");
    assert_eq!(err.message, NO_SERVICES);
}

#[tokio::test]
async fn opening_a_triage_creates_one_pending_record_per_active_catalogue_entry() {
    use business::domain::enums::GarageAttendanceOrigin;
    let db = mock()
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([Vec::<entity::garage_attendance_entity::Model>::new()])
        .append_query_results([[garage_service_row(1, "Diesel", "diesel"), garage_service_row(2, "WC", "wc")]])
        .append_exec_results([inserted(1), inserted(1), inserted(2)])
        .append_query_results([[attendance_row(1, "Open", Some(1))]])
        .append_query_results([[service_record_row(1, "diesel", "Pending")]])
        .append_query_results([[service_record_row(2, "wc", "Pending")]])
        .into_connection();
    let (attendance, services) =
        run_with_user(Some(owner(42)), garage_attendance_use_case(&db).open(Some(42), 10, Some(3), GarageAttendanceOrigin::Manual))
            .await
            .expect("opened");
    assert_eq!(attendance.manual_priority, Some(3));
    assert_eq!(services.len(), 2);
    let sql = format!("{:?}", log(db));
    assert_eq!(sql.matches("INSERT INTO `garage_service` ").count(), 2, "{sql}");
    assert!(sql.contains("\"diesel\"") && sql.contains("\"wc\""), "each record is keyed by its normalised name: {sql}");
}

#[tokio::test]
async fn marking_a_service_stamps_its_own_instant_and_writes_an_independent_audit_row() {
    let db = mock()
        .append_query_results([[attendance_row(1, "Open", Some(1))]]) // the triage
        .append_query_results([[service_record_row(1, "diesel", "Pending")]]) // its services
        .append_query_results([[service_record_row(1, "diesel", "Pending")]]) // the record by uuid
        .append_exec_results([inserted(1), inserted(7)])
        .append_query_results([[service_record_row(1, "diesel", "Performed")]])
        .append_query_results([[entity::garage_service_log_entity::Model {
            id: 7,
            uuid: string_to_bytes(UUID),
            tenant_id: Some(42),
            attendance_id: 1,
            vehicle_id: 10,
            service_model_id: 1,
            name_key: "diesel".into(),
            new_state: "Performed".into(),
            acted_by_user_id: Some(2),
            acted_at: at(),
            origin: "Employee".into(),
            created_at: at(),
            created_by: None,
            updated_at: at(),
            updated_by: None,
        }]])
        .into_connection();
    run_with_user(
        Some(owner(42)),
        garage_attendance_use_case(&db).mark_service(UUID.into(), UUID.into(), GarageServiceState::Performed, Some(2)),
    )
    .await
    .expect("marked");
    let sql = format!("{:?}", log(db));
    assert!(sql.contains("UPDATE `garage_service`") && sql.contains("Performed"), "{sql}");
    assert!(sql.contains("INSERT INTO `garage_service_log`"), "the audit row is written with the marking: {sql}");
}

#[tokio::test]
async fn a_closed_triage_cannot_be_marked_or_checked_out_again() {
    let closed = || {
        mock()
            .append_query_results([[attendance_row(1, "Finished", None)]])
            .append_query_results([[service_record_row(1, "diesel", "Performed")]])
            .into_connection()
    };
    let db = closed();
    let err = run_with_user(
        Some(owner(42)),
        garage_attendance_use_case(&db).mark_service(UUID.into(), UUID.into(), GarageServiceState::NotDone, None),
    )
    .await
    .expect_err("closed");
    assert_eq!(err.message, ATTENDANCE_CLOSED);
    let db = closed();
    let err = run_with_user(Some(owner(42)), garage_attendance_use_case(&db).checkout(UUID.into())).await.expect_err("closed");
    assert_eq!(err.message, ATTENDANCE_CLOSED);
}

#[tokio::test]
async fn checking_out_closes_the_triage_frees_its_slot_and_zeroes_the_priority() {
    let db = mock()
        .append_query_results([[attendance_row(1, "Open", Some(1))]])
        .append_query_results([[service_record_row(1, "diesel", "Performed")]])
        .append_exec_results([inserted(1)])
        .append_query_results([[entity::garage_attendance_entity::Model { manual_priority: None, ..attendance_row(1, "Finished", None) }]])
        .into_connection();
    let closed = run_with_user(Some(owner(42)), garage_attendance_use_case(&db).checkout(UUID.into())).await.expect("closed");
    assert_eq!(closed.manual_priority, None);
    assert_eq!(closed.active_marker, None);
    let sql = format!("{:?}", log(db));
    assert!(sql.contains("UPDATE `garage_attendance`") && sql.contains("Finished"), "{sql}");
}

// ------------------------------------------------------- EPIC-GA-03 presence stamps
use business::domain::enums::{PresenceEventKind, PresenceSource};
use business::use_cases::vehicle_presence_use_case::{ESTIMATE_CANNOT_STAMP, VehiclePresenceUseCase};

fn presence_use_case(db: &DatabaseConnection) -> VehiclePresenceUseCase {
    VehiclePresenceUseCase::new(
        business::gateway::vehicle_presence_event_gateway::VehiclePresenceEventGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
    )
}

fn presence_row(id: i64, kind: &str, hours_ago: i64) -> entity::vehicle_presence_event_entity::Model {
    entity::vehicle_presence_event_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        vehicle_id: 10,
        kind: kind.into(),
        occurred_at: chrono::Utc::now() - chrono::Duration::hours(hours_ago),
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

#[tokio::test]
async fn a_schedule_estimate_can_never_stamp_a_vehicle() {
    let db = mock().into_connection();
    let err = run_with_user(
        Some(owner(42)),
        presence_use_case(&db).record(
            Some(42),
            10,
            PresenceEventKind::Departure,
            chrono::Utc::now().naive_utc(),
            PresenceSource::ScheduleEstimate,
        ),
    )
    .await
    .expect_err("refused");
    assert_eq!(err.message, ESTIMATE_CANNOT_STAMP);
    assert!(log(db).is_empty(), "nothing is even read");
}

#[tokio::test]
async fn a_second_departure_with_no_arrival_between_leaves_the_stamp_untouched() {
    let db = mock()
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([[presence_row(2, "Departure", 3)]]) // the latest event
        .append_query_results([[presence_row(1, "Arrival", 9)]]) // snapshot: latest arrival
        .append_query_results([[presence_row(2, "Departure", 3)]]) // snapshot: latest departure
        .into_connection();
    let (recorded, snapshot) = run_with_user(
        Some(owner(42)),
        presence_use_case(&db).record(Some(42), 10, PresenceEventKind::Departure, chrono::Utc::now().naive_utc(), PresenceSource::Tracker),
    )
    .await
    .expect("answered");
    assert!(!recorded && snapshot.away);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

#[tokio::test]
async fn an_arrival_after_a_departure_is_recorded_and_the_vehicle_is_no_longer_away() {
    let db = mock()
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([[presence_row(2, "Departure", 3)]])
        .append_exec_results([inserted(3)])
        .append_query_results([[presence_row(3, "Arrival", 0)]]) // read-back of the insert
        .append_query_results([[presence_row(3, "Arrival", 0)]]) // snapshot: latest arrival
        .append_query_results([[presence_row(2, "Departure", 3)]]) // snapshot: latest departure
        .into_connection();
    let (recorded, snapshot) = run_with_user(
        Some(owner(42)),
        presence_use_case(&db).record(Some(42), 10, PresenceEventKind::Arrival, chrono::Utc::now().naive_utc(), PresenceSource::Tracker),
    )
    .await
    .expect("recorded");
    assert!(recorded && !snapshot.away);
    assert!(format!("{:?}", log(db)).contains("INSERT INTO `vehicle_presence_event`"));
}

// ------------------------------------------------------- EPIC-GA-05-S02 manual call to base
use business::use_cases::garage_call_use_case::{GarageCallUseCase, NOT_AWAY};

fn garage_call_use_case(db: &DatabaseConnection) -> GarageCallUseCase {
    GarageCallUseCase::new(
        business::gateway::garage_call_gateway::GarageCallGateway::new(db.clone()),
        business::gateway::vehicle_presence_event_gateway::VehiclePresenceEventGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
    )
}

fn call_row(id: i64, minutes_ago: i64, cancelled: bool) -> entity::garage_call_entity::Model {
    entity::garage_call_entity::Model {
        id,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        vehicle_id: 10,
        called_at: chrono::Utc::now() - chrono::Duration::minutes(minutes_ago),
        called_by_user_id: Some(2),
        cancelled_at: cancelled.then(chrono::Utc::now),
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

#[tokio::test]
async fn only_a_vehicle_away_from_base_can_be_called() {
    let db = mock()
        .append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
        .append_query_results([[presence_row(1, "Arrival", 2)]]) // last arrival: it is at base
        .append_query_results([Vec::<entity::vehicle_presence_event_entity::Model>::new()]) // never departed
        .into_connection();
    let err = run_with_user(Some(owner(42)), garage_call_use_case(&db).call(Some(42), 10, Some(2))).await.expect_err("refused");
    assert_eq!(err.message, NOT_AWAY);
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

#[tokio::test]
async fn a_call_to_an_away_vehicle_is_stamped_and_an_existing_call_is_not_stamped_again() {
    // Away: last departure (3 h ago) is later than the last arrival (9 h ago).
    let away = |db: MockDatabase| {
        db.append_query_results([[vehicle_row(10, 42, "ABC1D23", "Active")]])
            .append_query_results([[presence_row(1, "Arrival", 9)]])
            .append_query_results([[presence_row(2, "Departure", 3)]])
    };
    let db = away(mock())
        .append_query_results([Vec::<entity::garage_call_entity::Model>::new()]) // no call in force
        .append_exec_results([inserted(5)])
        .append_query_results([[call_row(5, 0, false)]])
        .into_connection();
    let (_, created) = run_with_user(Some(owner(42)), garage_call_use_case(&db).call(Some(42), 10, Some(2))).await.expect("called");
    assert!(created);
    assert!(format!("{:?}", log(db)).contains("INSERT INTO `garage_call`"));

    let db = away(mock())
        .append_query_results([[call_row(5, 30, false)]]) // a call already in force
        .append_query_results([[presence_row(1, "Arrival", 9)]]) // its arrival check: earlier than the call
        .into_connection();
    let (existing, created) = run_with_user(Some(owner(42)), garage_call_use_case(&db).call(Some(42), 10, Some(2))).await.expect("answered");
    assert!(!created && existing.id == Some(5));
    assert!(!format!("{:?}", log(db)).contains("INSERT"));
}

#[tokio::test]
async fn a_vehicles_arrival_after_the_call_ends_it_without_anyone_unmarking_it() {
    let db = mock()
        .append_query_results([[call_row(5, 120, false)]]) // called 2 h ago
        .append_query_results([[presence_row(3, "Arrival", 1)]]) // arrived 1 h ago, after the call
        .into_connection();
    assert!(run_with_user(Some(owner(42)), garage_call_use_case(&db).active(10)).await.expect("read").is_none());
}

// ------------------------------------------------------- EPIC-SC-04-S01 demand kind and days of week
#[tokio::test]
async fn a_stated_kind_needs_the_field_that_makes_it_a_days_work_and_unreadable_days_are_refused() {
    use business::domain::enums::DemandKind;
    use business::use_cases::transport_demand_use_case::{DAYS_OF_WEEK_UNREADABLE, KIND_NEEDS_ITS_DAYS};
    let db = mock().into_connection(); // every refusal happens before any query
    let refused = |demand: TransportDemand| {
        let db = db.clone();
        async move { run_with_user(Some(owner(42)), demand_use_case(&db).create(demand)).await.expect_err("refused").message }
    };
    let line = |days: Option<&str>| TransportDemand {
        demand_kind: Some(DemandKind::Line),
        days_of_week: days.map(str::to_string),
        ..new_demand(Some(42))
    };
    assert_eq!(refused(line(Some("Mon,Funday"))).await, DAYS_OF_WEEK_UNREADABLE);
    assert_eq!(refused(line(None)).await, KIND_NEEDS_ITS_DAYS, "a line without its days");
    let trip = TransportDemand { demand_kind: Some(DemandKind::OneOffTrip), ..new_demand(Some(42)) };
    assert_eq!(refused(trip).await, KIND_NEEDS_ITS_DAYS, "a one-off trip without its date");
}
