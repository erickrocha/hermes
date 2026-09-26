//! DEF-XF-10: the domain <-> entity mappers had no tests at all. They are the
//! seam every read and write crosses, so each is checked in both directions,
//! including the partial-ActiveModel fallback a failed `try_into_model` takes.
//! Pure: no database, no feature flags.

use business::commons::entity_mapper::EntityMapper;
use business::commons::functions::{bytes_para_string, string_to_bytes};
use business::domain::access_token::ClaimsBuilder;
use business::domain::business_plan::{BusinessPlan, BusinessPlanEntityMapper};
use business::domain::city::{City, CityEntityMapper};
use business::domain::customer::{Customer, CustomerEntityMapper};
use business::domain::customer_day_off::{CustomerDayOff, CustomerDayOffEntityMapper};
use business::domain::holiday::{Holiday, HolidayEntityMapper};
use business::domain::transport_demand::{TransportDemand, TransportDemandEntityMapper};
use business::domain::transport_demand_allocation::{
    TransportDemandAllocation, TransportDemandAllocationEntityMapper,
};
use business::domain::daily_schedule::{DailySchedule, DailyScheduleEntityMapper};
use business::domain::schedule_exception::{ScheduleException, ScheduleExceptionEntityMapper};
use business::domain::extra_trip::{ExtraTrip, ExtraTripEntityMapper};
use business::domain::km_evolution::{KmEvolution, KmEvolutionEntityMapper};
use business::domain::checklist_template::{ChecklistTemplate, ChecklistTemplateEntityMapper};
use business::domain::checklist_template_item::{
    ChecklistTemplateItem, ChecklistTemplateItemEntityMapper,
};
use business::domain::checklist_run::{ChecklistRun, ChecklistRunEntityMapper};
use business::domain::checklist_answer::{ChecklistAnswer, ChecklistAnswerEntityMapper};
use business::domain::work_order::{WorkOrder, WorkOrderEntityMapper};
use business::domain::work_order_item::{WorkOrderItem, WorkOrderItemEntityMapper};
use business::domain::maintenance_plan::{MaintenancePlan, MaintenancePlanEntityMapper};
use business::domain::service_type::{ServiceType, ServiceTypeEntityMapper};
use business::domain::priced_service::{PricedService, PricedServiceEntityMapper};
use business::domain::enums::GarageTagOrigin;
use business::domain::enums::ScheduleExceptionType;
use business::domain::enums::TripStatus;
use business::domain::enums::KmOrigin;
use business::domain::enums::ChecklistType;
use business::domain::enums::AnswerStatus;
use business::domain::enums::WorkOrderStatus;
use business::domain::enums::WorkOrderItemStatus;
use business::domain::enums::WorkOrderOrigin;
use business::domain::enums::MaintenancePlanStatus;
use business::domain::enums::Role;
use business::domain::enums::VehicleStatus;
use business::domain::province::{Province, ProvinceEntityMapper};
use business::domain::tenant::{Tenant, TenantEntityMapper};
use business::domain::user::{User, UserEntityMapper};
use business::domain::vehicle::{Vehicle, VehicleEntityMapper};
use chrono::{NaiveDate, TimeZone, Utc};
use entity::{
    business_plan_entity, city_entity, customer_day_off_entity, customer_entity,
    checklist_answer_entity, checklist_run_entity, checklist_template_entity,
    checklist_template_item_entity, maintenance_plan_entity, priced_service_entity,
    service_type_entity, work_order_entity, work_order_item_entity,
    daily_schedule_entity,
    extra_trip_entity, holiday_entity, km_evolution_entity, province_entity,
    schedule_exception_entity, tenant_entity, transport_demand_allocation_entity,
    transport_demand_entity, user_entity, vehicle_entity,
};
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
    assert_eq!(
        UserEntityMapper::build_active_models(users.clone()).len(),
        2
    );
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
    assert_eq!(
        active.business_name,
        ActiveValue::Set("Transportes LTDA".into())
    );
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

    assert_eq!(
        BusinessPlanEntityMapper::from_active_model(plan_model().into_active_model()),
        plan
    );
    assert_eq!(
        BusinessPlanEntityMapper::from_active_model(active).name,
        "Professional"
    );
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
    assert_eq!(
        ProvinceEntityMapper::from_active_model(active).name,
        "São Paulo"
    );

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
    assert_eq!(
        VehicleStatus::from_str(" Transit "),
        Ok(VehicleStatus::Transit)
    );
    assert!(VehicleStatus::from_str("Parked").is_err());
    assert!(
        VehicleStatus::from_str("active").is_err(),
        "the vocabulary is case-sensitive"
    );
    assert!(VehicleStatus::from_str("").is_err());
    assert_eq!(VehicleStatus::default(), VehicleStatus::Active);
}

/// `HRMS-941` (`C-023`): same shape as the vehicle-status round trip above,
/// minus a default -- `GarageTagOrigin` has none (see its own doc comment).
#[test]
fn garage_tag_origins_round_trip_and_an_unknown_one_is_rejected() {
    for origin in [
        GarageTagOrigin::Manual,
        GarageTagOrigin::Tracker,
        GarageTagOrigin::Automatic,
    ] {
        assert_eq!(GarageTagOrigin::from_str(&origin.to_string()), Ok(origin));
    }
    assert_eq!(
        GarageTagOrigin::from_str(" Tracker "),
        Ok(GarageTagOrigin::Tracker)
    );
    assert!(GarageTagOrigin::from_str("Automated").is_err());
    assert!(
        GarageTagOrigin::from_str("manual").is_err(),
        "the vocabulary is case-sensitive"
    );
    assert!(GarageTagOrigin::from_str("").is_err());
}

/// `HRMS-606` (`C-024`): same shape as the garage-tag-origin round trip
/// above.
#[test]
fn schedule_exception_types_round_trip_and_an_unknown_one_is_rejected() {
    for exception_type in [
        ScheduleExceptionType::Cancellation,
        ScheduleExceptionType::Deallocation,
        ScheduleExceptionType::Substitution,
    ] {
        assert_eq!(
            ScheduleExceptionType::from_str(&exception_type.to_string()),
            Ok(exception_type)
        );
    }
    assert!(ScheduleExceptionType::from_str("Reallocation").is_err());
    assert!(
        ScheduleExceptionType::from_str("cancellation").is_err(),
        "the vocabulary is case-sensitive"
    );
    assert!(ScheduleExceptionType::from_str("").is_err());
}

/// `HRMS-607` (`C-024`, `D-24(f)`): same shape as the other vocabulary
/// round trips in this file.
#[test]
fn trip_statuses_round_trip_and_an_unknown_one_is_rejected() {
    for status in [
        TripStatus::Scheduled,
        TripStatus::Conflict,
        TripStatus::PendingSchedule,
        TripStatus::Cancelled,
    ] {
        assert_eq!(TripStatus::from_str(&status.to_string()), Ok(status));
    }
    assert!(TripStatus::from_str("Completed").is_err());
    assert!(
        TripStatus::from_str("scheduled").is_err(),
        "the vocabulary is case-sensitive"
    );
    assert!(TripStatus::from_str("").is_err());
    assert_eq!(TripStatus::default(), TripStatus::Scheduled);
}

/// `HRMS-650` (`C-025`, `TRM-151`): same shape as the other vocabulary
/// round trips in this file. Seven values, not eight -- `"abastecimento"`
/// is `D-29`'s bypass defect, not carried forward.
#[test]
fn km_origins_round_trip_and_an_unknown_one_is_rejected() {
    for origin in [
        KmOrigin::InitialRegistration,
        KmOrigin::Manual,
        KmOrigin::WorkOrder,
        KmOrigin::DriverChecklist,
        KmOrigin::Garage,
        KmOrigin::TechnicalInspection,
        KmOrigin::Adjustment,
    ] {
        assert_eq!(KmOrigin::from_str(&origin.to_string()), Ok(origin));
    }
    assert!(KmOrigin::from_str("Fuel").is_err());
    assert!(KmOrigin::from_str("abastecimento").is_err());
    assert!(
        KmOrigin::from_str("manual").is_err(),
        "the vocabulary is case-sensitive"
    );
    assert!(KmOrigin::from_str("").is_err());
}

/// `HRMS-651` (`C-025`): same shape as `km_origins_round_trip_and_an_
/// unknown_one_is_rejected`, for the three-value checklist-type vocabulary.
#[test]
fn checklist_types_round_trip_and_an_unknown_one_is_rejected() {
    for checklist_type in [
        ChecklistType::Departure,
        ChecklistType::Return,
        ChecklistType::Standalone,
    ] {
        assert_eq!(
            ChecklistType::from_str(&checklist_type.to_string()),
            Ok(checklist_type)
        );
    }
    assert!(ChecklistType::from_str("Roundtrip").is_err());
    assert!(
        ChecklistType::from_str("departure").is_err(),
        "the vocabulary is case-sensitive"
    );
    assert!(ChecklistType::from_str("").is_err());
}

fn vehicle_model() -> vehicle_entity::Model {
    vehicle_entity::Model {
        id: 10,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        plate: "ABC1D23".into(),
        model: "Volvo FH".into(),
        status: "Maintenance".into(),
        tracker_device_id: Some(393333),
        prefix: None,
        vehicle_type: None,
        odometer_km: None,
        wheel_type: None,
        spare_tire_count: None,
        spare_tire_type: None,
        spare_tire_notes: None,
        garage_tag: None,
        garage_tag_origin: None,
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

fn transport_demand_allocation_model() -> transport_demand_allocation_entity::Model {
    transport_demand_allocation_entity::Model {
        id: 60,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        demand_id: 50,
        driver_id: 5,
        vehicle_id: 10,
        days_of_week: Some("MON,TUE,WED,THU,FRI".into()),
        start_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
        end_date: None,
        active: true,
        created_at: at(),
        created_by: Some("owner@example.com".into()),
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-604` (`C-024`): same round-trip coverage as `transport_demand_
/// round_trips_through_the_mapper`.
#[test]
fn transport_demand_allocation_round_trips_through_the_mapper() {
    let allocation: TransportDemandAllocation =
        TransportDemandAllocationEntityMapper::from_model(transport_demand_allocation_model());
    assert_eq!(allocation.id, Some(60));
    assert_eq!(allocation.demand_id, 50);
    assert_eq!(allocation.driver_id, 5);
    assert_eq!(allocation.vehicle_id, 10);
    assert!(allocation.active);

    let active_model =
        TransportDemandAllocationEntityMapper::build_active_model(allocation.clone());
    assert_eq!(active_model.driver_id, ActiveValue::Set(5));
    assert_eq!(active_model.vehicle_id, ActiveValue::Set(10));
    assert!(matches!(active_model.created_at, ActiveValue::NotSet));

    assert_eq!(
        TransportDemandAllocationEntityMapper::from_active_model(
            transport_demand_allocation_model().into_active_model()
        ),
        allocation
    );
}

fn daily_schedule_model() -> daily_schedule_entity::Model {
    daily_schedule_entity::Model {
        id: 70,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        demand_id: 50,
        driver_id: 5,
        vehicle_id: 10,
        date: NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(),
        start_time: chrono::NaiveTime::from_hms_opt(7, 0, 0),
        end_time: chrono::NaiveTime::from_hms_opt(15, 0, 0),
        notes: Some("Covers for regular driver".into()),
        created_at: at(),
        created_by: Some("owner@example.com".into()),
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-605` (`C-024`): same round-trip coverage as `transport_demand_
/// allocation_round_trips_through_the_mapper`.
#[test]
fn daily_schedule_round_trips_through_the_mapper() {
    let entry: DailySchedule = DailyScheduleEntityMapper::from_model(daily_schedule_model());
    assert_eq!(entry.id, Some(70));
    assert_eq!(entry.demand_id, 50);
    assert_eq!(entry.date, NaiveDate::from_ymd_opt(2026, 1, 5).unwrap());
    assert_eq!(entry.notes.as_deref(), Some("Covers for regular driver"));

    let active_model = DailyScheduleEntityMapper::build_active_model(entry.clone());
    assert_eq!(active_model.driver_id, ActiveValue::Set(5));
    assert!(matches!(active_model.created_at, ActiveValue::NotSet));

    assert_eq!(
        DailyScheduleEntityMapper::from_active_model(daily_schedule_model().into_active_model()),
        entry
    );
}

fn schedule_exception_model() -> schedule_exception_entity::Model {
    schedule_exception_entity::Model {
        id: 80,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        demand_id: 50,
        date: NaiveDate::from_ymd_opt(2026, 1, 6).unwrap(),
        exception_type: "Substitution".into(),
        new_driver_id: Some(6),
        new_vehicle_id: Some(11),
        reason: Some("Regular driver called in sick".into()),
        extra_trip_id: None,
        status: Some("Open".into()),
        created_at: at(),
        created_by: Some("owner@example.com".into()),
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-606` (`C-024`): same round-trip coverage as `daily_schedule_
/// round_trips_through_the_mapper`, plus the vocabulary degradation
/// `vehicle_statuses_round_trip_and_an_unknown_one_is_rejected` proves for
/// `VehicleStatus`.
#[test]
fn schedule_exception_round_trips_through_the_mapper() {
    let exception: ScheduleException =
        ScheduleExceptionEntityMapper::from_model(schedule_exception_model());
    assert_eq!(exception.id, Some(80));
    assert_eq!(exception.demand_id, 50);
    assert_eq!(exception.exception_type, ScheduleExceptionType::Substitution);
    assert_eq!(exception.new_driver_id, Some(6));
    assert_eq!(exception.reason.as_deref(), Some("Regular driver called in sick"));

    let active_model = ScheduleExceptionEntityMapper::build_active_model(exception.clone());
    assert_eq!(
        active_model.exception_type,
        ActiveValue::Set("Substitution".into())
    );
    assert!(matches!(active_model.created_at, ActiveValue::NotSet));

    assert_eq!(
        ScheduleExceptionEntityMapper::from_active_model(
            schedule_exception_model().into_active_model()
        ),
        exception
    );
}

#[test]
fn a_stored_exception_type_the_vocabulary_no_longer_knows_degrades_instead_of_panicking() {
    let mut model = schedule_exception_model();
    model.exception_type = "Rescheduling".into();
    assert_eq!(
        ScheduleExceptionEntityMapper::from_model(model).exception_type,
        ScheduleExceptionType::Cancellation
    );
}

// -------------------------------------------------------------- Customer
fn customer_model() -> customer_entity::Model {
    customer_entity::Model {
        id: 20,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        name: "Acme Logistics".into(),
        status: "Active".into(),
        notes: Some("Net 30".into()),
        created_at: at(),
        created_by: Some("owner@example.com".into()),
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-600` (`C-024`): same round-trip coverage `vehicle_round_trips_
/// through_the_mapper` gives `Vehicle` -- read, rebuild as an `ActiveModel`,
/// and recover through the partial fallback a failed `try_into_model` takes.
#[test]
fn customer_round_trips_through_the_mapper() {
    let customer: Customer = CustomerEntityMapper::from_model(customer_model());
    assert_eq!(customer.id, Some(20));
    assert_eq!(customer.uuid.as_deref(), Some(UUID));
    assert_eq!(customer.tenant_id, Some(42));
    assert_eq!(customer.name, "Acme Logistics");
    assert_eq!(customer.notes.as_deref(), Some("Net 30"));
    assert_eq!(customer.created_at, Some(at().naive_utc()));

    let active = CustomerEntityMapper::build_active_model(customer.clone());
    assert_eq!(active.name, ActiveValue::Set("Acme Logistics".into()));
    assert!(matches!(active.created_at, ActiveValue::NotSet));
    assert!(matches!(active.updated_at, ActiveValue::NotSet));

    assert_eq!(
        CustomerEntityMapper::from_active_model(customer_model().into_active_model()),
        customer
    );
}

#[test]
fn a_new_customer_leaves_id_and_uuid_for_the_database() {
    let mut customer = CustomerEntityMapper::from_model(customer_model());
    customer.id = None;
    customer.uuid = None;
    let active = CustomerEntityMapper::build_active_model(customer);
    assert!(matches!(active.id, ActiveValue::NotSet));
    assert!(matches!(active.uuid, ActiveValue::NotSet));
}

fn customer_day_off_model() -> customer_day_off_entity::Model {
    customer_day_off_entity::Model {
        id: 30,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        customer_id: 20,
        date: NaiveDate::from_ymd_opt(2026, 12, 25).unwrap(),
        reason: Some("National holiday".into()),
        created_at: at(),
        created_by: Some("owner@example.com".into()),
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-601` (`C-024`): same round-trip coverage as `customer_round_trips_
/// through_the_mapper`.
#[test]
fn customer_day_off_round_trips_through_the_mapper() {
    let day_off: CustomerDayOff =
        CustomerDayOffEntityMapper::from_model(customer_day_off_model());
    assert_eq!(day_off.id, Some(30));
    assert_eq!(day_off.customer_id, 20);
    assert_eq!(
        day_off.date,
        NaiveDate::from_ymd_opt(2026, 12, 25).unwrap()
    );
    assert_eq!(day_off.reason.as_deref(), Some("National holiday"));

    let active = CustomerDayOffEntityMapper::build_active_model(day_off.clone());
    assert_eq!(active.customer_id, ActiveValue::Set(20));
    assert!(matches!(active.created_at, ActiveValue::NotSet));

    assert_eq!(
        CustomerDayOffEntityMapper::from_active_model(
            customer_day_off_model().into_active_model()
        ),
        day_off
    );
}

// --------------------------------------------------------------- Holiday
fn holiday_model() -> holiday_entity::Model {
    holiday_entity::Model {
        id: 40,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        date: NaiveDate::from_ymd_opt(2026, 12, 25).unwrap(),
        name: "Christmas".into(),
        created_at: at(),
        created_by: Some("owner@example.com".into()),
        updated_at: at(),
        updated_by: None,
    }
}

// -------------------------------------------------------- TransportDemand
fn transport_demand_model() -> transport_demand_entity::Model {
    transport_demand_entity::Model {
        id: 50,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        demand_type: "RecurringLine".into(),
        customer_id: Some(20),
        line_name: Some("Morning shuttle".into()),
        shift_start: chrono::NaiveTime::from_hms_opt(6, 0, 0),
        shift_end: chrono::NaiveTime::from_hms_opt(8, 0, 0),
        days_of_week: Some("MON,TUE,WED,THU,FRI".into()),
        specific_date: None,
        priority: Some(1),
        preferred_vehicle_type: Some("Van".into()),
        preferred_vehicle_model: None,
        specific_driver_id: None,
        specific_vehicle_id: None,
        active: true,
        created_at: at(),
        created_by: Some("owner@example.com".into()),
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-603` (`C-024`): same round-trip coverage as `customer_round_trips_
/// through_the_mapper`.
#[test]
fn transport_demand_round_trips_through_the_mapper() {
    let demand: TransportDemand =
        TransportDemandEntityMapper::from_model(transport_demand_model());
    assert_eq!(demand.id, Some(50));
    assert_eq!(demand.demand_type, "RecurringLine");
    assert_eq!(demand.customer_id, Some(20));
    assert_eq!(demand.days_of_week.as_deref(), Some("MON,TUE,WED,THU,FRI"));
    assert!(demand.active);

    let active_model = TransportDemandEntityMapper::build_active_model(demand.clone());
    assert_eq!(
        active_model.demand_type,
        ActiveValue::Set("RecurringLine".into())
    );
    assert!(matches!(active_model.created_at, ActiveValue::NotSet));

    assert_eq!(
        TransportDemandEntityMapper::from_active_model(
            transport_demand_model().into_active_model()
        ),
        demand
    );
}

/// `HRMS-603` mapper fallback: a partial `ActiveModel` (the shape a failed
/// `try_into_model` produces) still recovers `active` as `true`, its
/// database default, rather than `false`.
#[test]
fn a_partial_transport_demand_active_model_defaults_active_to_true() {
    let partial = transport_demand_entity::ActiveModel {
        demand_type: ActiveValue::Set("RecurringLine".into()),
        ..Default::default()
    };
    let demand = TransportDemandEntityMapper::from_active_model(partial);
    assert!(demand.active);
}

/// `HRMS-602` (`C-024`): same round-trip coverage as `customer_round_trips_
/// through_the_mapper`.
#[test]
fn holiday_round_trips_through_the_mapper() {
    let holiday: Holiday = HolidayEntityMapper::from_model(holiday_model());
    assert_eq!(holiday.id, Some(40));
    assert_eq!(holiday.name, "Christmas");
    assert_eq!(holiday.date, NaiveDate::from_ymd_opt(2026, 12, 25).unwrap());

    let active = HolidayEntityMapper::build_active_model(holiday.clone());
    assert_eq!(active.name, ActiveValue::Set("Christmas".into()));
    assert!(matches!(active.created_at, ActiveValue::NotSet));

    assert_eq!(
        HolidayEntityMapper::from_active_model(holiday_model().into_active_model()),
        holiday
    );
}

// -------------------------------------------------------------- ExtraTrip
fn extra_trip_model() -> extra_trip_entity::Model {
    extra_trip_entity::Model {
        id: 90,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        order_code: "ORD-100".into(),
        trip_date: NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
        customer_id: Some(20),
        start_time: chrono::NaiveTime::from_hms_opt(6, 0, 0),
        return_date: None,
        return_time: None,
        destination: Some("Airport".into()),
        origin_city: Some("Campinas".into()),
        stops: None,
        preferred_vehicle_type: Some("Van".into()),
        driver_id: Some(5),
        second_driver_id: None,
        vehicle_id: Some(10),
        freight_value_cents: Some(150_000),
        payment: Some("Invoice".into()),
        status: "Scheduled".into(),
        origin: Some("Manual".into()),
        import_batch_id: None,
        imported_at: None,
        replaced_by_id: None,
        created_at: at(),
        created_by: Some("owner@example.com".into()),
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-607` (`C-024`, `D-24(f)`): same round-trip coverage as
/// `schedule_exception_round_trips_through_the_mapper`, plus the vocabulary
/// degradation `a_stored_exception_type_the_vocabulary_no_longer_knows_
/// degrades_instead_of_panicking` proves for `ScheduleExceptionType`.
#[test]
fn extra_trip_round_trips_through_the_mapper() {
    let trip: ExtraTrip = ExtraTripEntityMapper::from_model(extra_trip_model());
    assert_eq!(trip.id, Some(90));
    assert_eq!(trip.order_code, "ORD-100");
    assert_eq!(trip.trip_date, NaiveDate::from_ymd_opt(2026, 1, 10).unwrap());
    assert_eq!(trip.customer_id, Some(20));
    assert_eq!(trip.status, TripStatus::Scheduled);
    assert_eq!(trip.freight_value_cents, Some(150_000));

    let active = ExtraTripEntityMapper::build_active_model(trip.clone());
    assert_eq!(active.order_code, ActiveValue::Set("ORD-100".into()));
    assert_eq!(active.status, ActiveValue::Set("Scheduled".into()));
    assert!(matches!(active.created_at, ActiveValue::NotSet));

    assert_eq!(
        ExtraTripEntityMapper::from_active_model(extra_trip_model().into_active_model()),
        trip
    );
}

#[test]
fn a_stored_trip_status_the_vocabulary_no_longer_knows_degrades_instead_of_panicking() {
    let mut model = extra_trip_model();
    model.status = "Completed".into();
    assert_eq!(
        ExtraTripEntityMapper::from_model(model).status,
        TripStatus::Scheduled
    );
}

// ------------------------------------------------------------ KmEvolution
fn km_evolution_model() -> km_evolution_entity::Model {
    km_evolution_entity::Model {
        id: 91,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        vehicle_id: 10,
        km: 12_345.6,
        recorded_at: at(),
        origin: "Manual".into(),
        source_entity: None,
        source_entity_id: None,
        notes: Some("Odometer photo checked".into()),
        recorded_by_user_id: Some(7),
        created_at: at(),
        created_by: Some("owner@example.com".into()),
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-650` (`C-025`, `AD-041`): same round-trip coverage as the other
/// vocabulary-backed entities, plus the degradation
/// `a_stored_km_origin_the_vocabulary_no_longer_knows_degrades_instead_of_panicking`
/// proves for `KmOrigin`.
#[test]
fn km_evolution_round_trips_through_the_mapper() {
    let reading: KmEvolution = KmEvolutionEntityMapper::from_model(km_evolution_model());
    assert_eq!(reading.id, Some(91));
    assert_eq!(reading.vehicle_id, 10);
    assert_eq!(reading.km, 12_345.6);
    assert_eq!(reading.origin, KmOrigin::Manual);
    assert_eq!(reading.recorded_by_user_id, Some(7));

    let active = KmEvolutionEntityMapper::build_active_model(reading.clone());
    assert_eq!(active.vehicle_id, ActiveValue::Set(10));
    assert_eq!(active.origin, ActiveValue::Set("Manual".into()));
    assert!(matches!(active.created_at, ActiveValue::NotSet));

    assert_eq!(
        KmEvolutionEntityMapper::from_active_model(km_evolution_model().into_active_model()),
        reading
    );
}

#[test]
fn a_stored_km_origin_the_vocabulary_no_longer_knows_degrades_instead_of_panicking() {
    let mut model = km_evolution_model();
    model.origin = "Fuel".into();
    assert_eq!(
        KmEvolutionEntityMapper::from_model(model).origin,
        KmOrigin::Manual
    );
}

// ------------------------------------------------------- ChecklistTemplate
fn checklist_template_model() -> checklist_template_entity::Model {
    checklist_template_entity::Model {
        id: 92,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        name: "Departure inspection".into(),
        checklist_type: "Departure".into(),
        active: true,
        created_at: at(),
        created_by: Some("owner@example.com".into()),
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-651` (`C-025`): same round-trip coverage as the other
/// vocabulary-backed entities, plus the degradation
/// `a_stored_checklist_type_the_vocabulary_no_longer_knows_degrades_instead_
/// of_panicking` proves for `ChecklistType`.
#[test]
fn checklist_template_round_trips_through_the_mapper() {
    let template: ChecklistTemplate =
        ChecklistTemplateEntityMapper::from_model(checklist_template_model());
    assert_eq!(template.id, Some(92));
    assert_eq!(template.name, "Departure inspection");
    assert_eq!(template.checklist_type, ChecklistType::Departure);
    assert!(template.active);

    let active = ChecklistTemplateEntityMapper::build_active_model(template.clone());
    assert_eq!(active.name, ActiveValue::Set("Departure inspection".into()));
    assert_eq!(active.checklist_type, ActiveValue::Set("Departure".into()));
    assert!(matches!(active.created_at, ActiveValue::NotSet));

    assert_eq!(
        ChecklistTemplateEntityMapper::from_active_model(
            checklist_template_model().into_active_model()
        ),
        template
    );
}

#[test]
fn a_stored_checklist_type_the_vocabulary_no_longer_knows_degrades_instead_of_panicking() {
    let mut model = checklist_template_model();
    model.checklist_type = "Roundtrip".into();
    assert_eq!(
        ChecklistTemplateEntityMapper::from_model(model).checklist_type,
        ChecklistType::Standalone
    );
}

// -------------------------------------------------- ChecklistTemplateItem
fn checklist_template_item_model() -> checklist_template_item_entity::Model {
    checklist_template_item_entity::Model {
        id: 93,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        checklist_template_id: 92,
        description: "Check tyre pressure".into(),
        generates_work_order: true,
        created_at: at(),
        created_by: Some("owner@example.com".into()),
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-651` (`C-025`): same round-trip shape as `checklist_template_
/// round_trips_through_the_mapper` -- no vocabulary field here, so no
/// degradation case is needed.
#[test]
fn checklist_template_item_round_trips_through_the_mapper() {
    let item: ChecklistTemplateItem =
        ChecklistTemplateItemEntityMapper::from_model(checklist_template_item_model());
    assert_eq!(item.id, Some(93));
    assert_eq!(item.checklist_template_id, 92);
    assert_eq!(item.description, "Check tyre pressure");
    assert!(item.generates_work_order);

    let active = ChecklistTemplateItemEntityMapper::build_active_model(item.clone());
    assert_eq!(active.checklist_template_id, ActiveValue::Set(92));
    assert_eq!(active.generates_work_order, ActiveValue::Set(true));
    assert!(matches!(active.created_at, ActiveValue::NotSet));

    assert_eq!(
        ChecklistTemplateItemEntityMapper::from_active_model(
            checklist_template_item_model().into_active_model()
        ),
        item
    );
}

// ------------------------------------------------------------ ChecklistRun
fn checklist_run_model() -> checklist_run_entity::Model {
    checklist_run_entity::Model {
        id: 94,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        checklist_template_id: 92,
        driver_id: 5,
        vehicle_id: 10,
        checklist_type: "Departure".into(),
        odometer_km: 12_345.6,
        notes: Some("Full tank, no damage".into()),
        opening_checklist_id: None,
        created_at: at(),
        created_by: Some("owner@example.com".into()),
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-652` (`C-025`): same round-trip coverage as the other
/// vocabulary-backed entities, plus the degradation
/// `a_stored_checklist_run_type_the_vocabulary_no_longer_knows_degrades_
/// instead_of_panicking` proves for `ChecklistType`.
#[test]
fn checklist_run_round_trips_through_the_mapper() {
    let run: ChecklistRun = ChecklistRunEntityMapper::from_model(checklist_run_model());
    assert_eq!(run.id, Some(94));
    assert_eq!(run.driver_id, 5);
    assert_eq!(run.vehicle_id, 10);
    assert_eq!(run.checklist_type, ChecklistType::Departure);
    assert_eq!(run.odometer_km, 12_345.6);
    assert_eq!(run.opening_checklist_id, None);

    let active = ChecklistRunEntityMapper::build_active_model(run.clone());
    assert_eq!(active.driver_id, ActiveValue::Set(5));
    assert_eq!(active.checklist_type, ActiveValue::Set("Departure".into()));
    assert!(matches!(active.created_at, ActiveValue::NotSet));

    assert_eq!(
        ChecklistRunEntityMapper::from_active_model(checklist_run_model().into_active_model()),
        run
    );
}

#[test]
fn a_stored_checklist_run_type_the_vocabulary_no_longer_knows_degrades_instead_of_panicking() {
    let mut model = checklist_run_model();
    model.checklist_type = "Roundtrip".into();
    assert_eq!(
        ChecklistRunEntityMapper::from_model(model).checklist_type,
        ChecklistType::Standalone
    );
}

// --------------------------------------------------------- ChecklistAnswer
fn checklist_answer_model() -> checklist_answer_entity::Model {
    checklist_answer_entity::Model {
        id: 95,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        checklist_run_id: 94,
        checklist_template_item_id: 93,
        status: "NonConforming".into(),
        observation: Some("Left rear tyre low".into()),
        work_order_id: None,
        created_at: at(),
        created_by: Some("owner@example.com".into()),
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-654` (`C-025`): same round-trip coverage as the other
/// vocabulary-backed entities, plus the degradation
/// `a_stored_answer_status_the_vocabulary_no_longer_knows_degrades_instead_
/// of_panicking` proves for `AnswerStatus`.
#[test]
fn checklist_answer_round_trips_through_the_mapper() {
    let answer: ChecklistAnswer = ChecklistAnswerEntityMapper::from_model(checklist_answer_model());
    assert_eq!(answer.id, Some(95));
    assert_eq!(answer.checklist_run_id, 94);
    assert_eq!(answer.checklist_template_item_id, 93);
    assert_eq!(answer.status, AnswerStatus::NonConforming);
    assert_eq!(answer.work_order_id, None);

    let active = ChecklistAnswerEntityMapper::build_active_model(answer.clone());
    assert_eq!(active.checklist_run_id, ActiveValue::Set(94));
    assert_eq!(active.status, ActiveValue::Set("NonConforming".into()));
    assert!(matches!(active.created_at, ActiveValue::NotSet));

    assert_eq!(
        ChecklistAnswerEntityMapper::from_active_model(checklist_answer_model().into_active_model()),
        answer
    );
}

#[test]
fn a_stored_answer_status_the_vocabulary_no_longer_knows_degrades_instead_of_panicking() {
    let mut model = checklist_answer_model();
    model.status = "Ok".into();
    assert_eq!(
        ChecklistAnswerEntityMapper::from_model(model).status,
        AnswerStatus::NonConforming
    );
}

// ------------------------------------------------------------- WorkOrder
fn work_order_model() -> work_order_entity::Model {
    work_order_entity::Model {
        id: 96,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        vehicle_id: 10,
        opened_at: at(),
        odometer_km: 12_345.6,
        origin: "Manual".into(),
        checklist_run_id: None,
        maintenance_plan_id: None,
        service_type: Some("Brakes".into()),
        description: "Squeaking front brakes".into(),
        responsible: Some("Jane Mechanic".into()),
        status: "Open".into(),
        observation: None,
        external_service: false,
        supplier: None,
        invoice_number: None,
        invoice_value_cents: None,
        invoice_date: None,
        concluded_at: None,
        created_at: at(),
        created_by: Some("owner@example.com".into()),
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-700` (`C-026`): same round-trip coverage as the other
/// vocabulary-backed entities, plus the degradation
/// `a_stored_work_order_status_the_vocabulary_no_longer_knows_degrades_
/// instead_of_panicking` proves for `WorkOrderStatus`.
#[test]
fn work_order_round_trips_through_the_mapper() {
    let work_order: WorkOrder = WorkOrderEntityMapper::from_model(work_order_model());
    assert_eq!(work_order.id, Some(96));
    assert_eq!(work_order.vehicle_id, 10);
    assert_eq!(work_order.odometer_km, 12_345.6);
    assert_eq!(work_order.status, WorkOrderStatus::Open);
    assert_eq!(work_order.description, "Squeaking front brakes");
    assert_eq!(work_order.origin, WorkOrderOrigin::Manual);
    assert_eq!(work_order.checklist_run_id, None);

    let active = WorkOrderEntityMapper::build_active_model(work_order.clone());
    assert_eq!(active.vehicle_id, ActiveValue::Set(10));
    assert_eq!(active.status, ActiveValue::Set("Open".into()));
    assert!(matches!(active.created_at, ActiveValue::NotSet));

    assert_eq!(
        WorkOrderEntityMapper::from_active_model(work_order_model().into_active_model()),
        work_order
    );
}

#[test]
fn a_stored_work_order_status_the_vocabulary_no_longer_knows_degrades_instead_of_panicking() {
    let mut model = work_order_model();
    model.status = "Roundtrip".into();
    assert_eq!(
        WorkOrderEntityMapper::from_model(model).status,
        WorkOrderStatus::Open
    );
}

#[test]
fn work_order_statuses_round_trip_and_an_unknown_one_is_rejected() {
    for status in [
        WorkOrderStatus::Open,
        WorkOrderStatus::PartiallyResolved,
        WorkOrderStatus::AwaitingParts,
        WorkOrderStatus::Concluded,
        WorkOrderStatus::Cancelled,
    ] {
        assert_eq!(WorkOrderStatus::from_str(&status.to_string()), Ok(status));
    }
    assert!(WorkOrderStatus::from_str("Roundtrip").is_err());
    assert!(
        WorkOrderStatus::from_str("open").is_err(),
        "the vocabulary is case-sensitive"
    );
    assert!(WorkOrderStatus::from_str("").is_err());
    assert_eq!(WorkOrderStatus::default(), WorkOrderStatus::Open);
}

/// `EPIC-CK-03-S02` (`HRMS-653`): the origin vocabulary grown to its second
/// value now that a checklist can open a work order too.
#[test]
fn work_order_origins_round_trip_and_an_unknown_one_is_rejected() {
    for origin in [WorkOrderOrigin::Manual, WorkOrderOrigin::Checklist] {
        assert_eq!(WorkOrderOrigin::from_str(&origin.to_string()), Ok(origin));
    }
    assert!(WorkOrderOrigin::from_str("Roundtrip").is_err());
    assert!(
        WorkOrderOrigin::from_str("manual").is_err(),
        "the vocabulary is case-sensitive"
    );
    assert!(WorkOrderOrigin::from_str("").is_err());
}

// --------------------------------------------------------- WorkOrderItem
fn work_order_item_model() -> work_order_item_entity::Model {
    work_order_item_entity::Model {
        id: 97,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        work_order_id: 96,
        description: "Check brake pads".into(),
        item_type: Some("Brakes".into()),
        status: "Pending".into(),
        observation: None,
        resolved_by: None,
        resolved_at: None,
        resolution_description: None,
        created_at: at(),
        created_by: Some("owner@example.com".into()),
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-701` (`C-026`): same round-trip coverage as the other
/// vocabulary-backed entities, plus the degradation
/// `a_stored_work_order_item_status_the_vocabulary_no_longer_knows_
/// degrades_instead_of_panicking` proves for `WorkOrderItemStatus`.
#[test]
fn work_order_item_round_trips_through_the_mapper() {
    let item: WorkOrderItem = WorkOrderItemEntityMapper::from_model(work_order_item_model());
    assert_eq!(item.id, Some(97));
    assert_eq!(item.work_order_id, 96);
    assert_eq!(item.status, WorkOrderItemStatus::Pending);
    assert_eq!(item.description, "Check brake pads");

    let active = WorkOrderItemEntityMapper::build_active_model(item.clone());
    assert_eq!(active.work_order_id, ActiveValue::Set(96));
    assert_eq!(active.status, ActiveValue::Set("Pending".into()));
    assert!(matches!(active.created_at, ActiveValue::NotSet));

    assert_eq!(
        WorkOrderItemEntityMapper::from_active_model(work_order_item_model().into_active_model()),
        item
    );
}

#[test]
fn a_stored_work_order_item_status_the_vocabulary_no_longer_knows_degrades_instead_of_panicking() {
    let mut model = work_order_item_model();
    model.status = "Roundtrip".into();
    assert_eq!(
        WorkOrderItemEntityMapper::from_model(model).status,
        WorkOrderItemStatus::Pending
    );
}

#[test]
fn work_order_item_statuses_round_trip_and_an_unknown_one_is_rejected() {
    for status in [
        WorkOrderItemStatus::Pending,
        WorkOrderItemStatus::Resolved,
        WorkOrderItemStatus::Cancelled,
        WorkOrderItemStatus::AwaitingParts,
    ] {
        assert_eq!(WorkOrderItemStatus::from_str(&status.to_string()), Ok(status));
    }
    assert!(WorkOrderItemStatus::from_str("Roundtrip").is_err());
    assert!(
        WorkOrderItemStatus::from_str("pending").is_err(),
        "the vocabulary is case-sensitive"
    );
    assert!(WorkOrderItemStatus::from_str("").is_err());
    assert_eq!(WorkOrderItemStatus::default(), WorkOrderItemStatus::Pending);
}

// --------------------------------------------------------- MaintenancePlan
fn maintenance_plan_model() -> maintenance_plan_entity::Model {
    maintenance_plan_entity::Model {
        id: 400,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        vehicle_id: 10,
        date: NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
        planned_start: Some(at()),
        planned_end: Some(at()),
        status: "Scheduled".into(),
        affects_schedule: true,
        origin: "Manual".into(),
        created_at: at(),
        created_by: Some("owner@example.com".into()),
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-703` (`C-026`): same round-trip coverage as the other
/// vocabulary-backed entities, plus the degradation
/// `a_stored_maintenance_plan_status_the_vocabulary_no_longer_knows_
/// degrades_instead_of_panicking` proves for `MaintenancePlanStatus`.
#[test]
fn maintenance_plan_round_trips_through_the_mapper() {
    let plan: MaintenancePlan = MaintenancePlanEntityMapper::from_model(maintenance_plan_model());
    assert_eq!(plan.id, Some(400));
    assert_eq!(plan.vehicle_id, 10);
    assert_eq!(plan.status, MaintenancePlanStatus::Scheduled);
    assert!(plan.affects_schedule);

    let active = MaintenancePlanEntityMapper::build_active_model(plan.clone());
    assert_eq!(active.vehicle_id, ActiveValue::Set(10));
    assert_eq!(active.status, ActiveValue::Set("Scheduled".into()));
    assert!(matches!(active.created_at, ActiveValue::NotSet));

    assert_eq!(
        MaintenancePlanEntityMapper::from_active_model(maintenance_plan_model().into_active_model()),
        plan
    );
}

#[test]
fn a_stored_maintenance_plan_status_the_vocabulary_no_longer_knows_degrades_instead_of_panicking() {
    let mut model = maintenance_plan_model();
    model.status = "Roundtrip".into();
    assert_eq!(
        MaintenancePlanEntityMapper::from_model(model).status,
        MaintenancePlanStatus::Scheduled
    );
}

#[test]
fn maintenance_plan_statuses_round_trip_and_an_unknown_one_is_rejected() {
    for status in [
        MaintenancePlanStatus::Scheduled,
        MaintenancePlanStatus::Concluded,
        MaintenancePlanStatus::Cancelled,
        MaintenancePlanStatus::NotExecuted,
    ] {
        assert_eq!(MaintenancePlanStatus::from_str(&status.to_string()), Ok(status));
    }
    assert!(MaintenancePlanStatus::from_str("Roundtrip").is_err());
    assert!(
        MaintenancePlanStatus::from_str("scheduled").is_err(),
        "the vocabulary is case-sensitive"
    );
    assert!(MaintenancePlanStatus::from_str("").is_err());
    assert_eq!(MaintenancePlanStatus::default(), MaintenancePlanStatus::Scheduled);
}

// ----------------------------------------------------------- ServiceType
fn service_type_model() -> service_type_entity::Model {
    service_type_entity::Model {
        id: 600,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        code: "BRK".into(),
        name: "Brakes".into(),
        category: Some("Mechanical".into()),
        active: true,
        created_at: at(),
        created_by: Some("owner@example.com".into()),
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-704` (`C-026`): the same round-trip coverage every other entity
/// mapper in this file gets.
#[test]
fn service_type_round_trips_through_the_mapper() {
    let service_type: ServiceType = ServiceTypeEntityMapper::from_model(service_type_model());
    assert_eq!(service_type.id, Some(600));
    assert_eq!(service_type.code, "BRK");
    assert_eq!(service_type.name, "Brakes");
    assert!(service_type.active);

    let active = ServiceTypeEntityMapper::build_active_model(service_type.clone());
    assert_eq!(active.code, ActiveValue::Set("BRK".into()));
    assert!(matches!(active.created_at, ActiveValue::NotSet));

    assert_eq!(
        ServiceTypeEntityMapper::from_active_model(service_type_model().into_active_model()),
        service_type
    );
}

// --------------------------------------------------------- PricedService
fn priced_service_model() -> priced_service_entity::Model {
    priced_service_entity::Model {
        id: 700,
        uuid: string_to_bytes(UUID),
        tenant_id: Some(42),
        name: "Oil change".into(),
        category: Some("Preventive".into()),
        default_value_cents: Some(15_000),
        observation: None,
        created_at: at(),
        created_by: Some("owner@example.com".into()),
        updated_at: at(),
        updated_by: None,
    }
}

/// `HRMS-704` (`C-026`): the same round-trip coverage every other entity
/// mapper in this file gets.
#[test]
fn priced_service_round_trips_through_the_mapper() {
    let priced_service: PricedService = PricedServiceEntityMapper::from_model(priced_service_model());
    assert_eq!(priced_service.id, Some(700));
    assert_eq!(priced_service.name, "Oil change");
    assert_eq!(priced_service.default_value_cents, Some(15_000));

    let active = PricedServiceEntityMapper::build_active_model(priced_service.clone());
    assert_eq!(active.name, ActiveValue::Set("Oil change".into()));
    assert!(matches!(active.created_at, ActiveValue::NotSet));

    assert_eq!(
        PricedServiceEntityMapper::from_active_model(priced_service_model().into_active_model()),
        priced_service
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

    assert!(
        ClaimsBuilder::new().build().is_err(),
        "claims without a subject must not build"
    );
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
