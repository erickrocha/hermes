//! DEF-XF-10: the domain <-> entity mappers had no tests at all. They are the
//! seam every read and write crosses, so each is checked in both directions,
//! including the partial-ActiveModel fallback a failed `try_into_model` takes.
//! Pure: no database, no feature flags.

use business::commons::entity_mapper::EntityMapper;
use business::commons::functions::{bytes_para_string, string_to_bytes};
use business::domain::access_token::ClaimsBuilder;
use business::domain::business_plan::{BusinessPlan, BusinessPlanEntityMapper};
use business::domain::city::{City, CityEntityMapper};
use business::domain::enums::Role;
use business::domain::enums::VehicleStatus;
use business::domain::province::{Province, ProvinceEntityMapper};
use business::domain::tenant::{Tenant, TenantEntityMapper};
use business::domain::user::{User, UserEntityMapper};
use business::domain::vehicle::{Vehicle, VehicleEntityMapper};
use chrono::{NaiveDate, TimeZone, Utc};
use entity::{business_plan_entity, city_entity, province_entity, tenant_entity, user_entity, vehicle_entity};
use sea_orm::{ActiveValue, IntoActiveModel};
use std::str::FromStr;

const UUID: &str = "684db325-63cb-470a-aa09-45811dd1904a";

fn at() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 18, 12, 0, 0).unwrap()
}

// ---------------------------------------------------------------- functions

#[test]
fn uuid_bytes_round_trip() {
    let bytes = string_to_bytes(UUID);
    assert_eq!(bytes.len(), 16);
    assert_eq!(bytes_para_string(bytes), UUID);
}

#[test]
fn malformed_uuids_degrade_instead_of_panicking() {
    assert_eq!(string_to_bytes("not-a-uuid"), vec![0; 16]);
    assert_eq!(bytes_para_string(vec![1, 2, 3]), "");
}

// ---------------------------------------------------------------- Role

#[test]
fn roles_round_trip_through_their_names() {
    for role in [Role::SysAdmin, Role::TenantOwner, Role::TenantUser] {
        assert_eq!(Role::from_str(&role.to_string()), Ok(role));
    }
    assert_eq!(Role::from_str(" TenantOwner "), Ok(Role::TenantOwner));
    assert!(Role::from_str("Wizard").is_err());
    assert_eq!(Role::default(), Role::TenantUser);
}

// ---------------------------------------------------------------- User

fn user_model() -> user_entity::Model {
    user_entity::Model {
        id: 7,
        uuid: string_to_bytes(UUID),
        name: Some("Owner".into()),
        email: "owner@example.com".into(),
        password: "$argon2id$hash".into(),
        enabled: true,
        blocked_reason: Some("billing".into()),
        tenant_id: Some(42),
        role: "TenantOwner".into(),
        created_at: at(),
        created_by: Some("admin".into()),
        updated_at: at(),
        updated_by: Some("admin".into()),
    }
}

#[test]
fn user_from_model_carries_every_field() {
    let user = UserEntityMapper::from_model(user_model());
    assert_eq!(user.id, Some(7));
    assert_eq!(user.uuid.as_deref(), Some(UUID));
    assert_eq!(user.email, "owner@example.com");
    assert_eq!(user.role, Role::TenantOwner);
    assert_eq!(user.tenant_id, Some(42));
    assert_eq!(user.created_at, Some(at().naive_utc()));
    assert_eq!(user.updated_by.as_deref(), Some("admin"));
}

#[test]
fn an_unknown_stored_role_degrades_to_the_least_privileged_one() {
    let mut model = user_model();
    model.role = "Wizard".into();
    assert_eq!(UserEntityMapper::from_model(model).role, Role::TenantUser);
}

#[test]
fn user_active_model_never_touches_blocked_reason_or_audit_fields() {
    let active = UserEntityMapper::build_active_model(UserEntityMapper::from_model(user_model()));
    // Owned by the billing worker and by `stamp_audit` respectively.
    assert!(matches!(active.blocked_reason, ActiveValue::NotSet));
    assert!(matches!(active.created_at, ActiveValue::NotSet));
    assert!(matches!(active.created_by, ActiveValue::NotSet));
    assert_eq!(active.id, ActiveValue::Set(7));
    assert_eq!(active.role, ActiveValue::Set("TenantOwner".into()));
}

#[test]
fn a_new_user_leaves_id_and_uuid_for_the_database() {
    let mut user = UserEntityMapper::from_model(user_model());
    user.id = None;
    user.uuid = None;
    let active = UserEntityMapper::build_active_model(user);
    assert!(matches!(active.id, ActiveValue::NotSet));
    assert!(matches!(active.uuid, ActiveValue::NotSet));
}

#[test]
fn user_from_active_model_handles_complete_and_partial_models() {
    let complete = UserEntityMapper::from_active_model(user_model().into_active_model());
    assert_eq!(complete.id, Some(7));
    assert_eq!(complete.created_at, Some(at().naive_utc()));

    // Partial (audit columns not set): the field-by-field fallback.
    let partial = UserEntityMapper::build_active_model(UserEntityMapper::from_model(user_model()));
    let user = UserEntityMapper::from_active_model(partial);
    assert_eq!(user.email, "owner@example.com");
    assert_eq!(user.role, Role::TenantOwner);
    assert_eq!(user.created_at, None);
}

#[test]
fn from_models_maps_every_row() {
    let users: Vec<User> = UserEntityMapper::from_models(vec![user_model(), user_model()]);
    assert_eq!(users.len(), 2);
    assert_eq!(UserEntityMapper::build_active_models(users.clone()).len(), 2);
    assert_eq!(UserEntityMapper::build_active_model_vec(users).len(), 2);
    assert_eq!(
        UserEntityMapper::from_active_models(vec![user_model().into_active_model()]).len(),
        1
    );
}

// ---------------------------------------------------------------- Tenant

fn tenant_model() -> tenant_entity::Model {
    tenant_entity::Model {
        id: 1,
        uuid: string_to_bytes(UUID),
        business_name: "Transportes LTDA".into(),
        company_name: Some("Transmega".into()),
        tax_id: "11222333000181".into(),
        email: Some("ops@transmega.example".into()),
        phone: Some("+55 11 5555-0000".into()),
        website: Some("https://transmega.example".into()),
        address_line1: Some("Rua A, 1".into()),
        address_line2: None,
        locality: Some("Limeira".into()),
        administrative_area: Some("SP".into()),
        postal_code: Some("13480-000".into()),
        country_code: Some("BR".into()),
        business_plan_id: Some(3),
        created_at: at(),
        created_by: Some("admin".into()),
        updated_at: at(),
        updated_by: None,
    }
}

#[test]
fn tenant_round_trips_through_the_mapper() {
    let tenant: Tenant = TenantEntityMapper::from_model(tenant_model());
    assert_eq!(tenant.company_name.as_deref(), Some("Transmega"));
    assert_eq!(tenant.website.as_deref(), Some("https://transmega.example"));
    assert_eq!(tenant.business_plan_id, Some(3));

    let active = TenantEntityMapper::build_active_model(tenant.clone());
    assert_eq!(active.business_name, ActiveValue::Set("Transportes LTDA".into()));
    assert_eq!(active.id, ActiveValue::Set(1));

    let back = TenantEntityMapper::from_active_model(tenant_model().into_active_model());
    assert_eq!(back, tenant);

    let partial = TenantEntityMapper::from_active_model(active);
    assert_eq!(partial.tax_id, "11222333000181");
}

// ---------------------------------------------------------------- BusinessPlan

fn plan_model() -> business_plan_entity::Model {
    business_plan_entity::Model {
        id: 3,
        uuid: string_to_bytes(UUID),
        name: "Professional".into(),
        price_in_cents: 12_345,
        available_users: 10,
        period_days: 30,
        payment_date: NaiveDate::from_ymd_opt(2026, 9, 30).unwrap(),
        created_at: at(),
        created_by: None,
        updated_at: at(),
        updated_by: None,
    }
}

#[test]
fn business_plan_round_trips_through_the_mapper() {
    let plan: BusinessPlan = BusinessPlanEntityMapper::from_model(plan_model());
    assert_eq!(plan.price_in_cents, 12_345);
    assert_eq!(plan.uuid.as_deref(), Some(UUID));

    let active = BusinessPlanEntityMapper::build_active_model(plan.clone());
    assert_eq!(active.price_in_cents, ActiveValue::Set(12_345));

    assert_eq!(BusinessPlanEntityMapper::from_active_model(plan_model().into_active_model()), plan);
    assert_eq!(BusinessPlanEntityMapper::from_active_model(active).name, "Professional");
}

// ---------------------------------------------------------------- Province / City

#[test]
fn province_and_city_round_trip_through_their_mappers() {
    let province_model = province_entity::Model {
        id: 26,
        uuid: string_to_bytes(UUID),
        acronym: "SP".into(),
        name: "São Paulo".into(),
        country_code: "BR".into(),
    };
    let province: Province = ProvinceEntityMapper::from_model(province_model.clone());
    assert_eq!(province.acronym, "SP");
    let active = ProvinceEntityMapper::build_active_model(province.clone());
    assert_eq!(ProvinceEntityMapper::from_active_model(active), province);

    let mut new_province = province;
    new_province.id = None;
    new_province.uuid = None;
    let active = ProvinceEntityMapper::build_active_model(new_province);
    assert!(matches!(active.id, ActiveValue::NotSet));
    assert_eq!(ProvinceEntityMapper::from_active_model(active).name, "São Paulo");

    let city_model = city_entity::Model {
        id: 9,
        uuid: string_to_bytes(UUID),
        province_id: 26,
        name: "Limeira".into(),
    };
    let city: City = CityEntityMapper::from_model(city_model);
    assert_eq!(city.province_id, 26);
    let active = CityEntityMapper::build_active_model(city.clone());
    assert_eq!(CityEntityMapper::from_active_model(active), city);

    let mut new_city = city;
    new_city.id = None;
    new_city.uuid = None;
    assert_eq!(
        CityEntityMapper::from_active_model(CityEntityMapper::build_active_model(new_city)).name,
        "Limeira"
    );
}

// ---------------------------------------------------------------- Vehicle

/// EPIC-FO-01-S03 (HRMS-922, D-23(b)): the vocabulary round-trips, and a
/// status the platform does not know is an error rather than a silent
/// `Active`. That distinction is the whole reason the vocabulary was stated.
#[test]
fn vehicle_statuses_round_trip_and_an_unknown_one_is_rejected() {
    for status in [
        VehicleStatus::Active,
        VehicleStatus::Maintenance,
        VehicleStatus::Transit,
        VehicleStatus::Reserved,
        VehicleStatus::Inactive,
    ] {
        assert_eq!(VehicleStatus::from_str(&status.to_string()), Ok(status));
    }
    assert_eq!(VehicleStatus::from_str(" Transit "), Ok(VehicleStatus::Transit));
    assert!(VehicleStatus::from_str("Parked").is_err());
    assert!(VehicleStatus::from_str("active").is_err(), "the vocabulary is case-sensitive");
    assert!(VehicleStatus::from_str("").is_err());
    assert_eq!(VehicleStatus::default(), VehicleStatus::Active);
}

fn vehicle_model() -> vehicle_entity::Model {
    vehicle_entity::Model {
        id: 10,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        plate: "ABC1D23".into(),
        model: "Volvo FH".into(),
        status: "Maintenance".into(),
        created_at: at(),
        created_by: Some("owner@example.com".into()),
        updated_at: at(),
        updated_by: None,
    }
}

#[test]
fn vehicle_round_trips_through_the_mapper() {
    let vehicle: Vehicle = VehicleEntityMapper::from_model(vehicle_model());
    assert_eq!(vehicle.id, Some(10));
    assert_eq!(vehicle.uuid.as_deref(), Some(UUID));
    assert_eq!(vehicle.tenant_id, Some(42));
    assert_eq!(vehicle.plate, "ABC1D23");
    assert_eq!(vehicle.status, VehicleStatus::Maintenance);
    assert_eq!(vehicle.created_at, Some(at().naive_utc()));

    let active = VehicleEntityMapper::build_active_model(vehicle.clone());
    assert_eq!(active.plate, ActiveValue::Set("ABC1D23".into()));
    assert_eq!(active.status, ActiveValue::Set("Maintenance".into()));
    // Stamped by `impl_tenant_auditable_before_save!`, never by the mapper.
    assert!(matches!(active.created_at, ActiveValue::NotSet));
    assert!(matches!(active.created_by, ActiveValue::NotSet));
    assert!(matches!(active.updated_at, ActiveValue::NotSet));

    assert_eq!(
        VehicleEntityMapper::from_active_model(vehicle_model().into_active_model()),
        vehicle
    );

    // The partial-ActiveModel fallback, which is what a freshly built (not yet
    // saved) model takes.
    let partial = VehicleEntityMapper::from_active_model(active);
    assert_eq!(partial.plate, "ABC1D23");
    assert_eq!(partial.status, VehicleStatus::Maintenance);
    assert_eq!(partial.created_at, None);
}

#[test]
fn a_new_vehicle_leaves_id_and_uuid_for_the_database() {
    let mut vehicle = VehicleEntityMapper::from_model(vehicle_model());
    vehicle.id = None;
    vehicle.uuid = None;
    let active = VehicleEntityMapper::build_active_model(vehicle);
    assert!(matches!(active.id, ActiveValue::NotSet));
    assert!(matches!(active.uuid, ActiveValue::NotSet));
}

#[test]
fn a_stored_status_the_vocabulary_no_longer_knows_degrades_instead_of_panicking() {
    // Writes cannot create one (the edge refuses it), so this only ever
    // happens to a row written before a value was removed -- a read must not
    // be the thing that fails.
    let mut model = vehicle_model();
    model.status = "Impounded".into();
    assert_eq!(
        VehicleEntityMapper::from_model(model).status,
        VehicleStatus::Active
    );
}

// ---------------------------------------------------------------- Claims
#[test]
fn claims_builder_requires_its_fields() {
    let claims = ClaimsBuilder::new()
        .with_sub("owner@example.com")
        .exp(1_800_000_000)
        .uuid(UUID)
        .name("Owner")
        .user_id(7)
        .role(Role::TenantOwner)
        .tenant_id(Some(42))
        .build()
        .expect("complete claims build");
    assert_eq!(claims.sub, "owner@example.com");
    assert_eq!(claims.role, Role::TenantOwner);
    assert_eq!(claims.tenant_id, Some(42));

    assert!(ClaimsBuilder::new().build().is_err(), "claims without a subject must not build");
}

// ---------------------------------------------------------------- BusinessError

#[test]
fn business_errors_render_their_message() {
    use business::domain::business_error::BusinessError;
    let error = BusinessError::new("Tenant not found".into());
    assert_eq!(error.to_string(), "Tenant not found");
    assert!(format!("{error:?}").contains("Tenant not found"));
    let as_std: &dyn std::error::Error = &error;
    assert!(as_std.source().is_none());
}
