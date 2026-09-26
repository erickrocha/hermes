use crate::commons::exception_response::ExceptionResponse;
use crate::commons::i18n::{ErrorKey, Locale};
use std::str::FromStr;

use crate::endpoints::json::access_token_json::AccessTokenJson;
use crate::endpoints::json::city_json::CityJson;
use crate::endpoints::json::customer_json::CustomerJson;
use crate::endpoints::json::holiday_json::HolidayJson;
use crate::endpoints::json::province_json::ProvinceJson;
use crate::endpoints::json::tenant_json::TenantJson;
use crate::endpoints::json::user_json::UserJson;
use crate::endpoints::json::vehicle_json::VehicleJson;
use business::domain::access_token::AccessToken;
use business::domain::city::City;
use business::domain::customer::Customer;
use business::domain::holiday::Holiday;
use business::domain::enums::{
    AnswerStatus, ChecklistType, GarageTagOrigin, KmOrigin, Role, ScheduleExceptionType, TripStatus,
    VehicleStatus, WorkOrderItemStatus,
};
use business::domain::province::Province;
use business::domain::tenant::Tenant;
use business::domain::user::User;
use business::domain::vehicle::Vehicle;

pub trait Mapper<T, U> {
    fn json(t: T) -> U;

    fn domain(u: U) -> T;

    fn json_vec(u: Vec<T>) -> Vec<U> {
        u.into_iter().map(Self::json).collect()
    }
}

pub struct AccessTokenMapper {}

impl Mapper<AccessToken, AccessTokenJson> for AccessTokenMapper {
    fn json(access_token: AccessToken) -> AccessTokenJson {
        AccessTokenJson {
            access_token: access_token.access_token,
            token_type: access_token.token_type,
            expire_in: access_token.expire_in,
            refresh_token: access_token.refresh_token,
            email: access_token.email,
            uuid: access_token.uuid,
            name: access_token.name,
            user_id: access_token.user_id,
            role: access_token.role,
            tenant_id: access_token.tenant_id,
            tenant_uuid: access_token.tenant_uuid,
        }
    }

    fn domain(u: AccessTokenJson) -> AccessToken {
        AccessToken {
            access_token: u.access_token,
            token_type: u.token_type,
            expire_in: u.expire_in,
            refresh_token: u.refresh_token,
            email: u.email,
            uuid: u.uuid,
            name: u.name,
            user_id: u.user_id,
            role: u.role,
            tenant_id: u.tenant_id,
            tenant_uuid: u.tenant_uuid,
        }
    }
}

pub struct UserMapper {}

impl Mapper<User, UserJson> for UserMapper {
    fn json(user: User) -> UserJson {
        UserJson {
            id: user.id,
            uuid: user.uuid,
            name: user.name,
            email: user.email,
            password: None,
            enabled: user.enabled,
            role: user.role.to_string(),
            tenant_id: user.tenant_id,
            created_at: user.created_at,
            created_by: user.created_by,
            updated_at: user.updated_at,
            updated_by: user.updated_by,
        }
    }

    fn domain(u: UserJson) -> User {
        User {
            id: u.id,
            uuid: u.uuid,
            email: u.email,
            name: u.name,
            password: u.password.unwrap_or_default(),
            enabled: u.enabled,
            // Endpoints call `reject_unknown_role` first (DEF-XF-06); this
            // default is never reached with an unknown value.
            role: Role::from_str(&u.role).unwrap_or_default(),
            tenant_id: u.tenant_id,
            created_at: u.created_at,
            created_by: u.created_by,
            updated_at: u.updated_at,
            updated_by: u.updated_by,
        }
    }
}

pub struct TenantMapper {}

fn optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn canonical_address(canonical: Option<String>, alias: Option<String>) -> Option<String> {
    optional_text(canonical).or_else(|| optional_text(alias))
}

fn country_code(value: Option<String>) -> Option<String> {
    optional_text(value).map(|value| value.to_uppercase())
}

impl Mapper<Tenant, TenantJson> for TenantMapper {
    fn json(t: Tenant) -> TenantJson {
        let province = t.administrative_area.clone();
        let city = t.locality.clone();
        let zipcode = t.postal_code.clone();
        TenantJson {
            id: t.id,
            uuid: t.uuid,
            business_name: Some(t.business_name),
            company_name: t.company_name,
            tax_id: Some(t.tax_id),
            email: t.email,
            phone: t.phone,
            website: t.website,
            address_line1: t.address_line1,
            address_line2: t.address_line2,
            locality: t.locality,
            administrative_area: t.administrative_area,
            postal_code: t.postal_code,
            country_code: t.country_code,
            province,
            city,
            zipcode,
            business_plan_id: t.business_plan_id,
            created_at: t.created_at,
            created_by: t.created_by,
            updated_at: t.updated_at,
            updated_by: t.updated_by,
        }
    }

    fn domain(u: TenantJson) -> Tenant {
        let locality = canonical_address(u.locality, u.city);
        let administrative_area = canonical_address(u.administrative_area, u.province);
        let postal_code = canonical_address(u.postal_code, u.zipcode);
        Tenant {
            id: u.id,
            uuid: u.uuid,
            // DEF-XF-06: missing fields become empty and are rejected by the
            // use case's own validation (HRM-002) with a 400, instead of
            // panicking the handler and dropping the connection.
            business_name: u.business_name.unwrap_or_default(),
            company_name: u.company_name,
            tax_id: u.tax_id.unwrap_or_default(),
            email: optional_text(u.email),
            phone: optional_text(u.phone),
            website: u.website,
            address_line1: optional_text(u.address_line1),
            address_line2: optional_text(u.address_line2),
            locality,
            administrative_area,
            postal_code,
            country_code: country_code(u.country_code),
            // Never taken from the request body — see HRMS-224 and
            // `TenantUseCase::create`/`update`, which enforce this too.
            business_plan_id: None,
            created_at: u.created_at,
            created_by: u.created_by,
            updated_at: u.updated_at,
            updated_by: u.updated_by,
        }
    }
}

pub struct ProvinceMapper;
impl Mapper<Province, ProvinceJson> for ProvinceMapper {
    fn json(t: Province) -> ProvinceJson {
        ProvinceJson {
            id: t.id,
            uuid: t.uuid,
            acronym: t.acronym,
            name: t.name,
            country_code: t.country_code,
        }
    }

    fn domain(u: ProvinceJson) -> Province {
        Province {
            id: u.id,
            uuid: u.uuid,
            acronym: u.acronym,
            name: u.name,
            country_code: u.country_code,
        }
    }
}

pub struct CityMapper;
impl Mapper<City, CityJson> for CityMapper {
    fn json(t: City) -> CityJson {
        CityJson {
            id: t.id,
            uuid: t.uuid,
            province_id: t.province_id,
            name: t.name,
        }
    }

    fn domain(u: CityJson) -> City {
        City {
            id: u.id,
            uuid: u.uuid,
            province_id: u.province_id,
            name: u.name,
        }
    }
}

/// EPIC-FO-01-S05 (HRMS-924): the vehicle's HTTP shape <-> domain seam.
pub struct VehicleMapper;
impl Mapper<Vehicle, VehicleJson> for VehicleMapper {
    fn json(t: Vehicle) -> VehicleJson {
        VehicleJson {
            id: t.id,
            uuid: t.uuid,
            tenant_id: t.tenant_id,
            plate: t.plate,
            model: t.model,
            status: t.status.to_string(),
            tracker_device_id: t.tracker_device_id,
            prefix: t.prefix,
            vehicle_type: t.vehicle_type,
            odometer_km: t.odometer_km,
            wheel_type: t.wheel_type,
            spare_tire_count: t.spare_tire_count,
            spare_tire_type: t.spare_tire_type,
            spare_tire_notes: t.spare_tire_notes,
            garage_tag: t.garage_tag,
            garage_tag_origin: t.garage_tag_origin.map(|o| o.to_string()),
            created_at: t.created_at,
            created_by: t.created_by,
            updated_at: t.updated_at,
            updated_by: t.updated_by,
        }
    }

    fn domain(u: VehicleJson) -> Vehicle {
        Vehicle {
            id: u.id,
            uuid: u.uuid,
            tenant_id: u.tenant_id,
            plate: u.plate,
            model: u.model,
            // Endpoints call `reject_unknown_vehicle_status` first (HRMS-922);
            // this default is never reached with an unknown value.
            status: VehicleStatus::from_str(&u.status).unwrap_or_default(),
            tracker_device_id: u.tracker_device_id,
            prefix: u.prefix,
            vehicle_type: u.vehicle_type,
            odometer_km: u.odometer_km,
            wheel_type: u.wheel_type,
            spare_tire_count: u.spare_tire_count,
            spare_tire_type: u.spare_tire_type,
            spare_tire_notes: u.spare_tire_notes,
            garage_tag: u.garage_tag,
            // Endpoints call `reject_unknown_garage_tag_origin` first
            // (HRMS-941); `None` in means `None` out, never a guessed origin.
            garage_tag_origin: u
                .garage_tag_origin
                .and_then(|value| GarageTagOrigin::from_str(&value).ok()),
            created_at: u.created_at,
            created_by: u.created_by,
            updated_at: u.updated_at,
            updated_by: u.updated_by,
        }
    }
}

/// `EPIC-SC-01-S01` (`HRMS-600`): the customer registry's HTTP shape <-> domain
/// seam, same shape as `VehicleMapper` minus the status vocabulary.
pub struct CustomerMapper;
impl Mapper<Customer, CustomerJson> for CustomerMapper {
    fn json(t: Customer) -> CustomerJson {
        CustomerJson {
            id: t.id,
            uuid: t.uuid,
            tenant_id: t.tenant_id,
            name: t.name,
            status: t.status,
            notes: t.notes,
            created_at: t.created_at,
            created_by: t.created_by,
            updated_at: t.updated_at,
            updated_by: t.updated_by,
        }
    }

    fn domain(u: CustomerJson) -> Customer {
        Customer {
            id: u.id,
            uuid: u.uuid,
            tenant_id: u.tenant_id,
            name: u.name,
            status: u.status,
            notes: u.notes,
            created_at: u.created_at,
            created_by: u.created_by,
            updated_at: u.updated_at,
            updated_by: u.updated_by,
        }
    }
}

/// `EPIC-SC-01-S03` (`HRMS-602`): the holiday registry's HTTP shape <-> domain
/// seam.
pub struct HolidayMapper;
impl Mapper<Holiday, HolidayJson> for HolidayMapper {
    fn json(t: Holiday) -> HolidayJson {
        HolidayJson {
            id: t.id,
            uuid: t.uuid,
            tenant_id: t.tenant_id,
            date: t.date,
            name: t.name,
        }
    }

    fn domain(u: HolidayJson) -> Holiday {
        Holiday {
            id: u.id,
            uuid: u.uuid,
            tenant_id: u.tenant_id,
            date: u.date,
            name: u.name,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }
    }
}

/// DEF-XF-06 (owner, 2026-09-18): a role the platform doesn't know is refused
/// with 403 Forbidden -- never silently defaulted, never a panic.
pub fn reject_unknown_role(role: &str, locale: &Locale) -> Result<(), ExceptionResponse> {
    Role::from_str(role)
        .map(|_| ())
        .map_err(|_| ExceptionResponse::Forbidden(locale.clone(), ErrorKey::InvalidParameterValue))
}

/// EPIC-FO-01-S03 (HRMS-922, D-23(b)): a status outside the stated vocabulary
/// is refused, which is the whole point of stating one -- `Role`'s
/// `unwrap_or_default` behaviour would quietly file every misspelling as
/// `Active`, and two depots would go on using two spellings anyway.
///
/// 400 rather than `reject_unknown_role`'s 403: an unknown status is a
/// malformed field, not a permission the caller lacks. An empty status is
/// included -- registering a vehicle states its status (HRMS-920).
pub fn reject_unknown_vehicle_status(
    status: &str,
    locale: &Locale,
) -> Result<(), ExceptionResponse> {
    VehicleStatus::from_str(status)
        .map(|_| ())
        .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidVehicleStatus))
}

/// `HRMS-606` (`C-024`): the vocabulary the story itself names (cancellation
/// / de-allocation / substitution) is refused when unrecognised, the same
/// shape `reject_unknown_vehicle_status` uses -- required, so absent counts
/// as unknown too.
pub fn reject_unknown_schedule_exception_type(
    exception_type: &str,
    locale: &Locale,
) -> Result<(), ExceptionResponse> {
    ScheduleExceptionType::from_str(exception_type)
        .map(|_| ())
        .map_err(|_| {
            ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidScheduleExceptionType)
        })
}

/// `HRMS-650` (`C-025`, `TRM-151`): the seven-value kilometre-origin
/// vocabulary, refused when unrecognised -- required, so absent counts as
/// unknown too.
pub fn reject_unknown_km_origin(origin: &str, locale: &Locale) -> Result<(), ExceptionResponse> {
    KmOrigin::from_str(origin)
        .map(|_| ())
        .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidKmOrigin))
}

/// `HRMS-651` (`C-025`): the three-value checklist-type vocabulary, refused
/// when unrecognised -- required, so absent counts as unknown too.
pub fn reject_unknown_checklist_type(
    checklist_type: &str,
    locale: &Locale,
) -> Result<(), ExceptionResponse> {
    ChecklistType::from_str(checklist_type)
        .map(|_| ())
        .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidChecklistType))
}

/// `HRMS-654` (`C-025`, `TRM-115`/`TRM-116`): the two-value answer-status
/// vocabulary, refused when unrecognised -- required, so absent counts as
/// unknown too.
pub fn reject_unknown_answer_status(status: &str, locale: &Locale) -> Result<(), ExceptionResponse> {
    AnswerStatus::from_str(status)
        .map(|_| ())
        .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidAnswerStatus))
}

/// `HRMS-701` (`C-026`, `TRM-203`): the four-value work-order-item status
/// vocabulary, refused when unrecognised -- required, so absent counts as
/// unknown too.
pub fn reject_unknown_work_order_item_status(
    status: &str,
    locale: &Locale,
) -> Result<(), ExceptionResponse> {
    WorkOrderItemStatus::from_str(status)
        .map(|_| ())
        .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidWorkOrderItemStatus))
}

/// `HRMS-607` (`C-024`, `D-24(f)`): the four trip states `operacao-trm`'s
/// own inventory names, refused when unrecognised -- required, so absent
/// counts as unknown too.
pub fn reject_unknown_trip_status(status: &str, locale: &Locale) -> Result<(), ExceptionResponse> {
    TripStatus::from_str(status)
        .map(|_| ())
        .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidTripStatus))
}

/// `HRMS-941` (`C-023`): the same shape as `reject_unknown_vehicle_status`,
/// but absent is accepted -- a vehicle may simply have no garage tag yet.
/// Only a *sent-and-unrecognised* value is refused.
pub fn reject_unknown_garage_tag_origin(
    origin: Option<&str>,
    locale: &Locale,
) -> Result<(), ExceptionResponse> {
    match origin {
        None => Ok(()),
        Some(value) => GarageTagOrigin::from_str(value).map(|_| ()).map_err(|_| {
            ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidGarageTagOrigin)
        }),
    }
}

#[cfg(test)]
mod address_mapping_tests {
    use super::{canonical_address, country_code, optional_text};

    #[test]
    fn canonical_address_wins_and_legacy_alias_fills_missing_value() {
        assert_eq!(
            canonical_address(Some("  Campinas ".into()), Some("São Paulo".into())),
            Some("Campinas".into())
        );
        assert_eq!(
            canonical_address(Some("  ".into()), Some(" São Paulo ".into())),
            Some("São Paulo".into())
        );
    }

    #[test]
    fn optional_address_values_are_cleaned_and_country_is_uppercase() {
        assert_eq!(optional_text(Some("  ".into())), None);
        assert_eq!(country_code(Some(" br ".into())), Some("BR".into()));
    }
}

#[cfg(test)]
mod role_guard_tests {
    use super::reject_unknown_role;
    use crate::commons::exception_response::ExceptionResponse;
    use crate::commons::i18n::Locale;

    #[test]
    fn known_roles_pass_and_unknown_ones_are_forbidden() {
        let locale = Locale::from_accept_language(None);
        // Driver and Mechanic since EPIC-IA-09 (PD-026, D-22).
        for role in ["SysAdmin", "TenantOwner", "TenantUser", "Driver", "Mechanic"] {
            assert!(
                reject_unknown_role(role, &locale).is_ok(),
                "{role} must be accepted"
            );
        }
        for role in ["Wizard", "", "sysadmin", "driver", "Admin"] {
            assert!(
                matches!(
                    reject_unknown_role(role, &locale),
                    Err(ExceptionResponse::Forbidden(..))
                ),
                "{role:?} must be refused with 403"
            );
        }
    }
}

/// EPIC-FO-01-S03 (HRMS-922, D-23(b)).
#[cfg(test)]
mod vehicle_status_guard_tests {
    use super::{
        Mapper, VehicleMapper, reject_unknown_garage_tag_origin, reject_unknown_vehicle_status,
    };
    use crate::commons::exception_response::ExceptionResponse;
    use crate::commons::i18n::Locale;
    use crate::endpoints::json::vehicle_json::VehicleJson;
    use business::domain::enums::VehicleStatus;

    fn json(status: &str) -> VehicleJson {
        VehicleJson {
            tenant_id: Some(1),
            plate: "ABC1D23".to_string(),
            model: "Volvo FH".to_string(),
            status: status.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn the_five_stated_statuses_pass_and_anything_else_is_a_bad_request() {
        let locale = Locale::from_accept_language(None);
        for status in ["Active", "Maintenance", "Transit", "Reserved", "Inactive"] {
            assert!(
                reject_unknown_vehicle_status(status, &locale).is_ok(),
                "{status} must be accepted"
            );
        }
        // An unknown status is refused rather than filed as `Active`: a
        // silently defaulted value is exactly the two-spellings problem
        // HRMS-922 exists to prevent. An absent status counts as unknown.
        for status in ["Parked", "", "active", "Manutencao", "Sold"] {
            assert!(
                matches!(
                    reject_unknown_vehicle_status(status, &locale),
                    Err(ExceptionResponse::BadRequest(..))
                ),
                "{status:?} must be refused with 400"
            );
        }
    }

    #[test]
    fn garage_tag_origin_is_optional_but_not_arbitrary() {
        let locale = Locale::from_accept_language(None);
        assert!(
            reject_unknown_garage_tag_origin(None, &locale).is_ok(),
            "a vehicle with no tag yet must not be refused"
        );
        for origin in ["Manual", "Tracker", "Automatic"] {
            assert!(
                reject_unknown_garage_tag_origin(Some(origin), &locale).is_ok(),
                "{origin} must be accepted"
            );
        }
        for origin in ["Automated", "", "manual"] {
            assert!(
                matches!(
                    reject_unknown_garage_tag_origin(Some(origin), &locale),
                    Err(ExceptionResponse::BadRequest(..))
                ),
                "{origin:?} must be refused with 400"
            );
        }
    }

    #[test]
    fn a_vehicle_round_trips_through_the_json_mapper() {
        let domain = VehicleMapper::domain(json("Maintenance"));
        assert_eq!(domain.status, VehicleStatus::Maintenance);
        assert_eq!(domain.plate, "ABC1D23");
        assert_eq!(domain.tenant_id, Some(1));

        let back = VehicleMapper::json(domain);
        assert_eq!(back.status, "Maintenance");
        assert_eq!(back.model, "Volvo FH");
    }

    /// `HRMS-941` (`C-023`): the parity fields round-trip like every other
    /// field, and an absent `garageTagOrigin` stays absent rather than
    /// becoming a guessed value (unlike `status`, which has a default).
    #[test]
    fn parity_fields_round_trip_through_the_json_mapper() {
        let mut payload = json("Active");
        payload.prefix = Some("FR-12".to_string());
        payload.vehicle_type = Some("Truck".to_string());
        payload.odometer_km = Some(123_456.7);
        payload.wheel_type = Some("Dual".to_string());
        payload.spare_tire_count = Some(2);
        payload.spare_tire_type = Some("Steel".to_string());
        payload.spare_tire_notes = Some("Rear axle only".to_string());
        payload.garage_tag = Some("Ready".to_string());
        payload.garage_tag_origin = Some("Tracker".to_string());

        let domain = VehicleMapper::domain(payload);
        assert_eq!(domain.prefix.as_deref(), Some("FR-12"));
        assert_eq!(domain.odometer_km, Some(123_456.7));
        assert_eq!(domain.spare_tire_count, Some(2));
        assert_eq!(
            domain.garage_tag_origin,
            Some(business::domain::enums::GarageTagOrigin::Tracker)
        );

        let back = VehicleMapper::json(domain);
        assert_eq!(back.prefix.as_deref(), Some("FR-12"));
        assert_eq!(back.garage_tag.as_deref(), Some("Ready"));
        assert_eq!(back.garage_tag_origin.as_deref(), Some("Tracker"));

        let untagged = VehicleMapper::domain(json("Active"));
        assert_eq!(untagged.garage_tag_origin, None, "absent stays absent");
    }
}

/// `EPIC-SC-01-S01` (`HRMS-600`, `C-024`).
#[cfg(test)]
mod customer_mapper_tests {
    use super::{CustomerMapper, Mapper};
    use crate::endpoints::json::customer_json::CustomerJson;

    #[test]
    fn a_customer_round_trips_through_the_json_mapper() {
        let payload = CustomerJson {
            tenant_id: Some(1),
            name: "Acme Logistics".to_string(),
            status: "Active".to_string(),
            notes: Some("Net 30".to_string()),
            ..Default::default()
        };

        let domain = CustomerMapper::domain(payload);
        assert_eq!(domain.name, "Acme Logistics");
        assert_eq!(domain.notes.as_deref(), Some("Net 30"));

        let back = CustomerMapper::json(domain);
        assert_eq!(back.name, "Acme Logistics");
        assert_eq!(back.status, "Active");
        assert_eq!(back.notes.as_deref(), Some("Net 30"));
    }
}

/// `EPIC-SC-01-S03` (`HRMS-602`, `C-024`).
#[cfg(test)]
mod holiday_mapper_tests {
    use super::{HolidayMapper, Mapper};
    use crate::endpoints::json::holiday_json::HolidayJson;
    use chrono::NaiveDate;

    #[test]
    fn a_holiday_round_trips_through_the_json_mapper() {
        let payload = HolidayJson {
            tenant_id: Some(1),
            date: NaiveDate::from_ymd_opt(2026, 12, 25).unwrap(),
            name: "Christmas".to_string(),
            ..Default::default()
        };

        let domain = HolidayMapper::domain(payload);
        assert_eq!(domain.name, "Christmas");

        let back = HolidayMapper::json(domain);
        assert_eq!(back.name, "Christmas");
        assert_eq!(back.date, NaiveDate::from_ymd_opt(2026, 12, 25).unwrap());
    }
}

/// `EPIC-SC-03-S01` (`HRMS-607`, `C-024`, `D-24(f)`).
#[cfg(test)]
mod trip_status_guard_tests {
    use super::reject_unknown_trip_status;
    use crate::commons::exception_response::ExceptionResponse;
    use crate::commons::i18n::Locale;

    #[test]
    fn the_four_named_statuses_pass_and_anything_else_is_a_bad_request() {
        let locale = Locale::from_accept_language(None);
        for status in ["Scheduled", "Conflict", "PendingSchedule", "Cancelled"] {
            assert!(
                reject_unknown_trip_status(status, &locale).is_ok(),
                "{status} must be accepted"
            );
        }
        for status in ["Completed", "", "scheduled"] {
            assert!(
                matches!(
                    reject_unknown_trip_status(status, &locale),
                    Err(ExceptionResponse::BadRequest(..))
                ),
                "{status:?} must be refused with 400"
            );
        }
    }
}

/// `EPIC-SC-02-S04` (`HRMS-606`, `C-024`).
#[cfg(test)]
mod schedule_exception_type_guard_tests {
    use super::reject_unknown_schedule_exception_type;
    use crate::commons::exception_response::ExceptionResponse;
    use crate::commons::i18n::Locale;

    #[test]
    fn the_three_named_types_pass_and_anything_else_is_a_bad_request() {
        let locale = Locale::from_accept_language(None);
        for exception_type in ["Cancellation", "Deallocation", "Substitution"] {
            assert!(
                reject_unknown_schedule_exception_type(exception_type, &locale).is_ok(),
                "{exception_type} must be accepted"
            );
        }
        for exception_type in ["Rescheduling", "", "cancellation"] {
            assert!(
                matches!(
                    reject_unknown_schedule_exception_type(exception_type, &locale),
                    Err(ExceptionResponse::BadRequest(..))
                ),
                "{exception_type:?} must be refused with 400"
            );
        }
    }
}

/// `EPIC-CK-02-S01` (`HRMS-651`, `C-025`).
#[cfg(test)]
mod checklist_type_guard_tests {
    use super::reject_unknown_checklist_type;
    use crate::commons::exception_response::ExceptionResponse;
    use crate::commons::i18n::Locale;

    #[test]
    fn the_three_named_types_pass_and_anything_else_is_a_bad_request() {
        let locale = Locale::from_accept_language(None);
        for checklist_type in ["Departure", "Return", "Standalone"] {
            assert!(
                reject_unknown_checklist_type(checklist_type, &locale).is_ok(),
                "{checklist_type} must be accepted"
            );
        }
        for checklist_type in ["Roundtrip", "", "departure"] {
            assert!(
                matches!(
                    reject_unknown_checklist_type(checklist_type, &locale),
                    Err(ExceptionResponse::BadRequest(..))
                ),
                "{checklist_type:?} must be refused with 400"
            );
        }
    }
}

/// `EPIC-CK-04-S01` (`HRMS-654`, `C-025`, `TRM-115`/`TRM-116`).
#[cfg(test)]
mod answer_status_guard_tests {
    use super::reject_unknown_answer_status;
    use crate::commons::exception_response::ExceptionResponse;
    use crate::commons::i18n::Locale;

    #[test]
    fn the_two_named_statuses_pass_and_anything_else_is_a_bad_request() {
        let locale = Locale::from_accept_language(None);
        for status in ["Conforming", "NonConforming"] {
            assert!(
                reject_unknown_answer_status(status, &locale).is_ok(),
                "{status} must be accepted"
            );
        }
        for status in ["Ok", "", "conforming"] {
            assert!(
                matches!(
                    reject_unknown_answer_status(status, &locale),
                    Err(ExceptionResponse::BadRequest(..))
                ),
                "{status:?} must be refused with 400"
            );
        }
    }
}

/// `EPIC-MT-01-S02` (`HRMS-701`, `C-026`, `TRM-203`).
#[cfg(test)]
mod work_order_item_status_guard_tests {
    use super::reject_unknown_work_order_item_status;
    use crate::commons::exception_response::ExceptionResponse;
    use crate::commons::i18n::Locale;

    #[test]
    fn the_four_named_statuses_pass_and_anything_else_is_a_bad_request() {
        let locale = Locale::from_accept_language(None);
        for status in ["Pending", "Resolved", "Cancelled", "AwaitingParts"] {
            assert!(
                reject_unknown_work_order_item_status(status, &locale).is_ok(),
                "{status} must be accepted"
            );
        }
        for status in ["Aberta", "", "pending"] {
            assert!(
                matches!(
                    reject_unknown_work_order_item_status(status, &locale),
                    Err(ExceptionResponse::BadRequest(..))
                ),
                "{status:?} must be refused with 400"
            );
        }
    }
}

/// `EPIC-CK-01-S01` (`HRMS-650`, `C-025`, `TRM-151`).
#[cfg(test)]
mod km_origin_guard_tests {
    use super::reject_unknown_km_origin;
    use crate::commons::exception_response::ExceptionResponse;
    use crate::commons::i18n::Locale;

    #[test]
    fn the_seven_named_origins_pass_and_anything_else_is_a_bad_request() {
        let locale = Locale::from_accept_language(None);
        for origin in [
            "InitialRegistration",
            "Manual",
            "WorkOrder",
            "DriverChecklist",
            "Garage",
            "TechnicalInspection",
            "Adjustment",
        ] {
            assert!(
                reject_unknown_km_origin(origin, &locale).is_ok(),
                "{origin} must be accepted"
            );
        }
        for origin in ["Fuel", "", "manual"] {
            assert!(
                matches!(
                    reject_unknown_km_origin(origin, &locale),
                    Err(ExceptionResponse::BadRequest(..))
                ),
                "{origin:?} must be refused with 400"
            );
        }
    }
}
