use std::str::FromStr;
use crate::commons::exception_response::ExceptionResponse;
use crate::commons::i18n::{ErrorKey, Locale};

use crate::endpoints::json::access_token_json::AccessTokenJson;
use crate::endpoints::json::city_json::CityJson;
use crate::endpoints::json::province_json::ProvinceJson;
use crate::endpoints::json::tenant_json::TenantJson;
use crate::endpoints::json::user_json::UserJson;
use crate::endpoints::json::vehicle_json::VehicleJson;
use business::domain::access_token::AccessToken;
use business::domain::city::City;
use business::domain::enums::{Role, VehicleStatus};
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

pub struct CityMapper;impl Mapper<City, CityJson> for CityMapper {
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
            created_at: u.created_at,
            created_by: u.created_by,
            updated_at: u.updated_at,
            updated_by: u.updated_by,
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
pub fn reject_unknown_vehicle_status(status: &str, locale: &Locale) -> Result<(), ExceptionResponse> {
    VehicleStatus::from_str(status)
        .map(|_| ())
        .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidVehicleStatus))
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
        for role in ["SysAdmin", "TenantOwner", "TenantUser"] {
            assert!(reject_unknown_role(role, &locale).is_ok(), "{role} must be accepted");
        }
        for role in ["Wizard", "", "sysadmin", "Driver"] {
            assert!(
                matches!(reject_unknown_role(role, &locale), Err(ExceptionResponse::Forbidden(..))),
                "{role:?} must be refused with 403"
            );
        }
    }
}

/// EPIC-FO-01-S03 (HRMS-922, D-23(b)).
#[cfg(test)]
mod vehicle_status_guard_tests {
    use super::{reject_unknown_vehicle_status, Mapper, VehicleMapper};
    use crate::commons::exception_response::ExceptionResponse;
    use crate::commons::i18n::Locale;
    use crate::endpoints::json::vehicle_json::VehicleJson;
    use business::domain::enums::VehicleStatus;

    fn json(status: &str) -> VehicleJson {
        VehicleJson {
            id: None,
            uuid: None,
            tenant_id: Some(1),
            plate: "ABC1D23".to_string(),
            model: "Volvo FH".to_string(),
            status: status.to_string(),
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
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
    fn a_vehicle_round_trips_through_the_json_mapper() {
        let domain = VehicleMapper::domain(json("Maintenance"));
        assert_eq!(domain.status, VehicleStatus::Maintenance);
        assert_eq!(domain.plate, "ABC1D23");
        assert_eq!(domain.tenant_id, Some(1));

        let back = VehicleMapper::json(domain);
        assert_eq!(back.status, "Maintenance");
        assert_eq!(back.model, "Volvo FH");
    }
}
