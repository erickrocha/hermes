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
use entity::audit::{run_as_platform, run_with_user, AuditUser};
use entity::{business_plan_entity, city_entity, province_entity, tenant_entity, user_entity};
use sea_orm::{DatabaseBackend, DatabaseConnection, MockDatabase, MockExecResult, Transaction};

const UUID: &str = "684db325-63cb-470a-aa09-45811dd1904a";

fn at() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 18, 12, 0, 0).unwrap()
}

fn mock() -> MockDatabase {
    MockDatabase::new(DatabaseBackend::MySql)
}

fn inserted(id: u64) -> MockExecResult {
    MockExecResult { last_insert_id: id, rows_affected: 1 }
}

fn log(db: DatabaseConnection) -> Vec<Transaction> {
    db.into_transaction_log()
}

fn owner(tenant_id: i64) -> AuditUser {
    AuditUser { id: 2, email: "owner@example.com".into(), tenant_id: Some(tenant_id), enforce_tenant: true }
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
    city_entity::Model { id, uuid: string_to_bytes(UUID), province_id, name: name.into() }
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

fn user_row(id: i64, email: &str, role: &str, tenant_id: Option<i64>, password: &str) -> user_entity::Model {
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
    Province { id: None, uuid: None, acronym: acronym.into(), name: name.into(), country_code: country.into() }
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
    assert!(sql.contains(r#"String(Some("ZZ"))"#) && sql.contains(r#"String(Some("BR"))"#), "stored values must be normalised: {sql}");
    assert!(!sql.contains(r#"Some(" zz ")"#) && !sql.contains(r#"Some("br")"#), "raw CSV values leaked: {sql}");
}

#[tokio::test]
async fn saving_a_duplicate_acronym_is_refused_before_writing() {
    let db = mock().append_query_results([[province_row(26, "SP", "BR")]]).into_connection();
    let err = ProvinceUseCase::new(ProvinceGateway::new(db.clone()))
        .save(province("SP", "Another São Paulo", "BR"))
        .await
        .expect_err("a second SP in BR must be refused");
    assert!(err.message.contains("already exists"));
    assert_eq!(log(db).len(), 1, "only the lookup ran; nothing was written");
}

#[tokio::test]
async fn an_invalid_province_never_reaches_the_database() {
    let db = mock().into_connection();
    let err = ProvinceUseCase::new(ProvinceGateway::new(db.clone()))
        .save(province("SP", "", "BR"))
        .await
        .expect_err("blank name");
    assert!(err.message.contains("name is required"));
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
        .import(vec![province("zz", "Testlandia", "br"), province("SP", "São Paulo", "BR")])
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
    assert!(err.message.contains("row 2: name is required"), "{}", err.message);
    assert!(err.message.contains("row 3: duplicate acronym"), "{}", err.message);
    assert!(!err.message.contains("row 2: duplicate"), "the duplicate is row 3, not row 2: {}", err.message);
    assert!(log(db).is_empty(), "a rejected batch must not touch the database");
}

#[tokio::test]
async fn province_reads() {
    let db = mock()
        .append_query_results([[province_row(26, "SP", "BR")]])
        .append_query_results([Vec::<province_entity::Model>::new()])
        .append_query_results([[province_row(26, "SP", "BR")]])
        .append_query_results([vec![province_row(26, "SP", "BR"), province_row(27, "RJ", "BR")]])
        .into_connection();
    let uc = ProvinceUseCase::new(ProvinceGateway::new(db));
    assert_eq!(uc.find_by_id(26).await.unwrap().acronym, "SP");
    assert!(uc.find_by_id(999).await.is_err(), "a missing id is an error, not a default");
    assert_eq!(uc.find_by_uuid(UUID.into()).await.unwrap().id, Some(26));
    assert_eq!(uc.find_by_country_code("BR".into()).await.unwrap().len(), 2);
}

// ================================================================ cities

fn city(name: &str, province_id: i64) -> City {
    City { id: None, uuid: None, province_id, name: name.into() }
}

#[tokio::test]
async fn city_save_and_its_guards() {
    let db = mock()
        .append_query_results([Vec::<city_entity::Model>::new()])
        .append_exec_results([inserted(9)])
        .append_query_results([[city_row(9, 26, "Limeira")]])
        .append_query_results([[city_row(9, 26, "Limeira")]]) // duplicate check for 2nd save
        .into_connection();
    let uc = CityUseCase::new(CityGateway::new(db));
    assert_eq!(uc.save(city(" Limeira ", 26)).await.unwrap().id, Some(9));
    let dup = uc.save(city("Limeira", 26)).await.expect_err("same name, same province");
    assert!(dup.message.contains("already exists"));
    assert!(uc.save(city("Orphan", 0)).await.is_err(), "a city needs a province");
}

#[tokio::test]
async fn city_import_is_all_or_nothing_and_idempotent_by_name() {
    let empty = mock().into_connection();
    let err = CityUseCase::new(CityGateway::new(empty.clone()))
        .import(vec![city("A", 26), city("A", 26)])
        .await
        .expect_err("duplicate in file");
    assert!(err.message.contains("row 2: duplicate city"), "{}", err.message);
    assert!(log(empty).is_empty());

    let db = mock()
        .append_query_results([[city_row(9, 26, "Limeira")]])
        .append_exec_results([inserted(9)])
        .append_query_results([[city_row(9, 26, "Limeira")]])
        .into_connection();
    let outcome = CityUseCase::new(CityGateway::new(db)).import(vec![city("Limeira", 26)]).await.unwrap();
    assert_eq!((outcome.created, outcome.updated), (0, 1), "re-importing updates, never duplicates");
}

#[tokio::test]
async fn city_reads() {
    let db = mock()
        .append_query_results([[city_row(9, 26, "Limeira")]])
        .append_query_results([[city_row(9, 26, "Limeira")]])
        .append_query_results([vec![city_row(9, 26, "Limeira"), city_row(10, 26, "Campinas")]])
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
    assert_eq!(uc.create(plan(" Professional ", 10_000)).await.unwrap().id, Some(3));
    assert_eq!(uc.find_by_id(3).await.unwrap().name, "Professional");
    assert_eq!(uc.find_by_uuid(UUID).await.unwrap().id, Some(3));
    assert_eq!(uc.find_all().await.unwrap().len(), 2);
    assert_eq!(uc.update(3, plan("Professional", 12_000)).await.unwrap().id, Some(3));
}

#[tokio::test]
async fn business_plan_rules_are_checked_before_any_write() {
    let db = mock().into_connection();
    let uc = BusinessPlanUseCase::new(BusinessPlanGateway::new(db.clone()));
    assert!(uc.create(plan("", 100)).await.is_err(), "name required");
    assert!(uc.create(plan("Paid", -1)).await.is_err(), "negative price refused");
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn a_plan_in_use_cannot_be_deleted() {
    let db = mock()
        .append_query_results([[plan_row(3)]])
        .append_query_results([[tenant_row(1)]]) // a tenant references it
        .into_connection();
    let err = BusinessPlanUseCase::new(BusinessPlanGateway::new(db)).delete(3).await.expect_err("in use");
    assert!(err.message.to_lowercase().contains("assigned"), "{}", err.message);
}

#[tokio::test]
async fn an_unused_plan_is_deleted_and_a_missing_one_is_reported() {
    let db = mock()
        .append_query_results([[plan_row(3)]])
        .append_query_results([Vec::<tenant_entity::Model>::new()])
        .append_exec_results([MockExecResult { last_insert_id: 0, rows_affected: 1 }])
        .append_query_results([Vec::<business_plan_entity::Model>::new()])
        .into_connection();
    let uc = BusinessPlanUseCase::new(BusinessPlanGateway::new(db));
    uc.delete(3).await.expect("unused plan deletes");
    assert!(uc.delete(404).await.is_err());
}

// ================================================================ tenants

fn tenant(business_name: &str, company: Option<&str>, tax_id: &str, country: Option<&str>) -> Tenant {
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
        .create(tenant("Transportes LTDA", Some("Transmega"), "11.222.333/0001-81", Some("BR")))
        .await
        .expect("valid tenant");
    assert_eq!(created.id, Some(1));
    let sql = format!("{:?}", log(db));
    assert!(sql.contains("11222333000181"), "CNPJ must be stored as digits: {sql}");
    assert!(!sql.contains("Some(99)"), "a caller-supplied plan must never be written: {sql}");
}

#[tokio::test]
async fn tenant_creation_rules() {
    let db = mock().into_connection();
    let uc = TenantUseCase::new(TenantGateway::new(db.clone()));
    assert!(uc.create(tenant("", None, "11222333000181", Some("BR"))).await.is_err(), "HRM-002");
    assert!(uc.create(tenant("   ", None, "11222333000181", Some("BR"))).await.is_err(), "DEF-TP-03: a name of blanks");
    assert!(uc.create(tenant("X", None, "11222333000181", Some("BRA"))).await.is_err(), "HRM-003");
    assert!(uc.create(tenant("X", None, "11222333000181", None)).await.is_err(), "country required");
    assert!(uc.create(tenant("X", None, "123", Some("BR"))).await.is_err(), "invalid CNPJ");
    // DEF-TP-01: a country with no validator must still require an identifier.
    assert!(uc.create(tenant("X", None, "", Some("US"))).await.is_err(), "tax id required");
    assert!(uc.create(tenant("X", None, "   ", Some("CA"))).await.is_err(), "blank tax id");
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

    let missing = uc.update(404, tenant("X", Some("Y"), "11222333000181", Some("BR"))).await;
    assert!(missing.as_ref().is_err_and(|e| e.is_not_found()), "an unknown tenant is a not-found, not a generic failure");

    // DEF-TP-03: the business name is validated on update too, and blanking it
    // out is refused.
    assert!(uc.update(1, tenant("  ", Some("Y"), "11222333000181", Some("BR"))).await.is_err());
    // DEF-TP-01: and so is emptying the tax identifier.
    assert!(uc.update(1, tenant("X", Some("Y"), "", Some("BR"))).await.is_err());
    // DEF-TP-02: company name is optional on create, so it cannot be mandatory
    // on update -- a tenant onboarded with only its required fields must stay
    // editable.
    assert!(
        uc.update(1, tenant("X", None, "11222333000181", Some("BR"))).await.is_ok(),
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
        .update(1, tenant("Transportes LTDA", Some("Renamed"), "11222333000181", Some("BR")))
        .await
        .expect("valid update");
    assert_eq!(updated.business_plan_id, Some(3));
    let sql = format!("{:?}", log(db));
    assert!(sql.contains("Renamed"), "{sql}");
    assert!(!sql.contains("Some(99)"), "the plan from the request body must be ignored: {sql}");
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
    assert_eq!(uc.find_by_id(1).await.unwrap().company_name.as_deref(), Some("Transmega"));
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
        .append_query_results([[user_row(5, "new@example.com", "TenantUser", Some(42), "$argon2id$x")]])
        .into_connection();
    let created = run_with_user(
        Some(owner(42)),
        UserUseCase::new(UserGateway::new(db.clone()))
            .create(new_user("new@example.com", "Correct#Horse9", Role::TenantUser, Some(42))),
    )
    .await
    .expect("valid user");
    assert_eq!(created.id, Some(5));
    let sql = format!("{:?}", log(db));
    assert!(!sql.contains("Correct#Horse9"), "plaintext password reached the database: {sql}");
    assert!(sql.contains("$argon2id$"), "password must be stored as Argon2id: {sql}");
}

#[tokio::test]
async fn user_creation_rules_are_checked_before_any_write() {
    let db = mock().into_connection();
    let uc = UserUseCase::new(UserGateway::new(db.clone()));
    assert!(uc.create(new_user("", "Correct#Horse9", Role::TenantUser, Some(42))).await.is_err());
    assert!(uc.create(new_user("a@b.c", "", Role::TenantUser, Some(42))).await.is_err());
    assert!(uc.create(new_user("a@b.c", "short", Role::TenantUser, Some(42))).await.is_err());
    assert!(log(db).is_empty());
}

#[tokio::test]
async fn a_user_write_without_any_tenant_scope_is_refused() {
    // D-05/D-06: outside a request the scope is Denied, and Denied refuses the
    // write instead of storing a tenant-less row.
    let db = mock().into_connection();
    let err = UserUseCase::new(UserGateway::new(db))
        .create(new_user("new@example.com", "Correct#Horse9", Role::TenantUser, Some(42)))
        .await
        .expect_err("no scope, no write");
    assert!(!err.message.is_empty());
}

#[tokio::test]
async fn updating_a_user_without_a_password_keeps_the_stored_hash() {
    let existing = user_row(5, "u@example.com", "TenantUser", Some(42), "$argon2id$stored");
    let db = mock()
        .append_query_results([[existing.clone()]])
        .append_exec_results([inserted(5)])
        .append_query_results([[existing]])
        .into_connection();
    run_with_user(
        Some(owner(42)),
        UserUseCase::new(UserGateway::new(db.clone())).update(5, new_user("u@example.com", "", Role::TenantUser, Some(42))),
    )
    .await
    .expect("update without password");
    let sql = format!("{:?}", log(db));
    assert!(sql.contains("$argon2id$stored"), "HRM-035: an omitted password keeps the old one: {sql}");
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
    let wrong = run_with_user(Some(owner(42)), uc.change_password(5, "nope".into(), "Brand#New99".into())).await;
    assert!(wrong.is_err(), "wrong current password must be refused");
    let ok = run_with_user(Some(owner(42)), uc.change_password(5, "Current#Pass1".into(), "Brand#New99".into())).await;
    assert!(ok.is_ok(), "{:?}", ok.err().map(|e| e.message));
    assert!(uc.change_password(5, "x".into(), "short".into()).await.is_err(), "policy applies to the new one");
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
    assert_eq!(run_with_user(Some(owner(42)), uc.find_by_id(5)).await.unwrap().email, "u@example.com");
    assert!(run_with_user(Some(owner(42)), uc.find_by_id(404)).await.is_err());
    assert_eq!(run_with_user(Some(owner(42)), uc.find_all()).await.unwrap().len(), 2);
    assert_eq!(uc.find_all_by_tenant_id(42).await.unwrap().len(), 1);
    assert!(UserUseCase::find_by_email(&db, "u@example.com".into()).await.is_some());
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
        .append_query_results([[user_row(1, "admin@hermes.test", "SysAdmin", None, "$argon2id$x")]])
        .append_query_results([[user_row(1, "admin@hermes.test", "SysAdmin", None, "$argon2id$x")]]) // second boot
        .into_connection();
    let first = run_as_platform(UserUseCase::seed_sysadmin(&db)).await.expect("seeded");
    assert_eq!(first.role, Role::SysAdmin);
    assert_eq!(first.tenant_id, None);
    let again = run_as_platform(UserUseCase::seed_sysadmin(&db)).await.expect("found");
    assert_eq!(again.id, Some(1), "a second boot must not create a second administrator");
}

// ================================================================ paging (PD-028)

use sea_orm::{DbErr, Value};
use std::collections::BTreeMap;

/// The paginator asks for the total first, then the page.
fn count(n: i64) -> Vec<Vec<BTreeMap<&'static str, Value>>> {
    vec![vec![BTreeMap::from([("num_items", Value::BigInt(Some(n)))])]]
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
        .append_query_results([vec![user_row(5, "u@example.com", "TenantUser", Some(42), "h")]])
        .into_connection();

    let (tenants, total) = TenantUseCase::new(TenantGateway::new(db.clone())).find_page(0, 25, Some("Trans")).await.unwrap();
    assert_eq!((tenants.len(), total), (2, 51));
    let (plans, total) = BusinessPlanUseCase::new(BusinessPlanGateway::new(db.clone())).find_page(0, 25, Some("Pro")).await.unwrap();
    assert_eq!((plans.len(), total), (1, 3));
    let (provinces, total) = ProvinceUseCase::new(ProvinceGateway::new(db.clone())).find_page(0, 25, Some("SP")).await.unwrap();
    assert_eq!((provinces.len(), total), (1, 78));
    let (cities, total) = CityUseCase::new(CityGateway::new(db.clone())).find_page(3, 25, Some("Lim")).await.unwrap();
    assert_eq!((cities.len(), total), (1, 5000));
    let (users, total) = run_with_user(Some(owner(42)), UserUseCase::new(UserGateway::new(db.clone())).find_page(0, 25, Some("u@"))).await.unwrap();
    assert_eq!((users.len(), total), (1, 2));

    let sql = format!("{:?}", log(db));
    assert!(sql.contains("LIKE"), "a search term must reach the database as a filter: {sql}");
    assert!(sql.contains("OFFSET"), "page 3 must be an OFFSET query, not a client-side slice: {sql}");
}

#[tokio::test]
async fn a_tenant_owners_user_page_is_filtered_to_their_tenant() {
    let db = mock()
        .append_query_results(count(0))
        .append_query_results([Vec::<user_entity::Model>::new()])
        .into_connection();
    run_with_user(Some(owner(42)), UserUseCase::new(UserGateway::new(db.clone())).find_page(0, 25, None))
        .await
        .unwrap();
    let sql = format!("{:?}", log(db));
    assert!(sql.contains("tenant_id") && sql.contains("42"), "paging must not escape tenant scope: {sql}");
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
    assert!(provinces.import(vec![province("SP", "São Paulo", "BR")]).await.is_err());

    let cities = CityUseCase::new(CityGateway::new(failing(6)));
    assert!(cities.find_by_id(1).await.is_err());
    assert!(cities.find_by_uuid(UUID.into()).await.is_err());
    assert!(cities.find_all().await.is_err());
    assert!(cities.find_by_province_id(26).await.is_err());
    assert!(cities.find_page(0, 25, None).await.is_err());
    assert!(cities.import(vec![city("Limeira", 26)]).await.is_err());

    let users = UserUseCase::new(UserGateway::new(failing(4)));
    assert!(run_with_user(Some(owner(42)), users.find_by_id(1)).await.is_err());
    assert!(run_with_user(Some(owner(42)), users.find_all()).await.is_err());
    assert!(users.find_all_by_tenant_id(42).await.is_err());
    assert!(run_with_user(Some(owner(42)), users.find_page(0, 25, None)).await.is_err());
    assert!(UserUseCase::find_by_email(&failing(1), "u@example.com".into()).await.is_none());
}

#[tokio::test]
async fn failed_writes_are_reported() {
    let write_fails = || {
        mock()
            .append_exec_errors([DbErr::Custom("constraint violation".into())])
            .into_connection()
    };
    let tenant_err = TenantUseCase::new(TenantGateway::new(write_fails()))
        .create(tenant("Transportes LTDA", None, "11222333000181", Some("BR")))
        .await;
    assert!(tenant_err.is_err());

    let plan_err = BusinessPlanUseCase::new(BusinessPlanGateway::new(write_fails())).create(plan("Pro", 100)).await;
    assert!(plan_err.is_err());

    let user_err = run_with_user(
        Some(owner(42)),
        UserUseCase::new(UserGateway::new(write_fails())).create(new_user("n@example.com", "Correct#Horse9", Role::TenantUser, Some(42))),
    )
    .await;
    assert!(user_err.is_err());

    let persisted = run_as_platform(UserUseCase::persist(
        write_fails(),
        new_user("n@example.com", "Correct#Horse9", Role::TenantUser, Some(42)),
    ))
    .await;
    assert!(persisted.is_none());
    assert!(UserUseCase::persist(mock().into_connection(), new_user("", "", Role::TenantUser, None)).await.is_none());
}

#[tokio::test]
async fn persisting_a_user_hashes_the_password() {
    let db = mock()
        .append_exec_results([inserted(6)])
        .append_query_results([[user_row(6, "p@example.com", "TenantUser", Some(42), "$argon2id$x")]])
        .into_connection();
    let saved = run_as_platform(UserUseCase::persist(
        db.clone(),
        new_user("p@example.com", "Correct#Horse9", Role::TenantUser, Some(42)),
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
        UserUseCase::new(UserGateway::new(db.clone())).update(5, new_user("u@example.com", "Brand#New99", Role::TenantUser, Some(42))),
    )
    .await
    .expect("update ignores the password");
    let sql = format!("{:?}", log(db));
    assert!(!sql.contains("Brand#New99"), "a supplied password must never reach the database: {sql}");
    assert!(sql.contains("$argon2id$old"), "the stored hash must be written back unchanged: {sql}");

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
        .update(5, new_user("u@example.com", "short", Role::TenantUser, Some(42))),
    )
    .await;
    assert!(short.is_ok(), "the password field is inert on update");

    assert!(
        run_with_user(Some(owner(42)), UserUseCase::new(UserGateway::new(failing(1))).update(5, new_user("u@example.com", "", Role::TenantUser, Some(42))))
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
        .append_query_results([[user_row(1, "admin@hermes.test", "SysAdmin", None, "$argon2id$x")]])
        .into_connection();
    let seeded = run_as_platform(UserUseCase::seed_sysadmin(&db)).await.expect("re-pointed");
    assert_eq!(seeded.id, Some(1), "the existing administrator is updated, not duplicated");
    let sql = format!("{:?}", log(db));
    assert!(sql.contains("UPDATE"), "{sql}");
    assert!(!sql.contains("INSERT"), "no second administrator: {sql}");
}
