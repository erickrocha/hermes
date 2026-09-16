use std::str::FromStr;

use crate::endpoints::json::access_token_json::AccessTokenJson;
use crate::endpoints::json::city_json::CityJson;
use crate::endpoints::json::province_json::ProvinceJson;
use crate::endpoints::json::tenant_json::TenantJson;
use crate::endpoints::json::tenant_plan_json::TenantPlanJson;
use crate::endpoints::json::user_json::UserJson;
use business::domain::access_token::AccessToken;
use business::domain::city::City;
use business::domain::enums::Role;
use business::domain::province::Province;
use business::domain::tenant::Tenant;
use business::domain::tenant_plan::TenantPlan;
use business::domain::user::User;

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
            first_login: access_token.first_login,
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
            first_login: u.first_login,
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
            first_login: user.first_login,
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
            first_login: u.first_login,
            role: Role::from_str(&u.role).unwrap(),
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
            payment_grace_days: t.payment_grace_days,
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
            business_name: u.business_name.unwrap(),
            company_name: u.company_name,
            tax_id: u.tax_id.unwrap(),
            email: optional_text(u.email),
            phone: optional_text(u.phone),
            website: u.website,
            address_line1: optional_text(u.address_line1),
            address_line2: optional_text(u.address_line2),
            locality,
            administrative_area,
            postal_code,
            country_code: country_code(u.country_code),
            payment_grace_days: u.payment_grace_days,
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

pub struct TenantPlanMapper;
impl Mapper<TenantPlan, TenantPlanJson> for TenantPlanMapper {
    fn json(t: TenantPlan) -> TenantPlanJson {
        TenantPlanJson {
            id: t.id,
            uuid: t.uuid,
            tenant_id: t.tenant_id,
            business_plan_id: t.business_plan_id,
            payment_date: t.payment_date,
            active: t.active,
        }
    }

    fn domain(u: TenantPlanJson) -> TenantPlan {
        TenantPlan {
            id: u.id,
            uuid: u.uuid,
            tenant_id: u.tenant_id,
            business_plan_id: u.business_plan_id,
            payment_date: u.payment_date,
            active: u.active,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }
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
