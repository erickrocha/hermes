use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::NaiveDateTime;
use entity::tenant_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set, TryIntoModel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tenant {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub business_name: String,
    pub company_name: Option<String>,
    pub tax_id: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub website: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub locality: Option<String>,
    pub administrative_area: Option<String>,
    pub postal_code: Option<String>,
    pub country_code: Option<String>,
    /// Days a customer keeps app access after a charge falls due.
    pub payment_grace_days: Option<i32>,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
}

pub struct TenantEntityMapper {}

impl EntityMapper<Tenant, Model, ActiveModel> for TenantEntityMapper {
    fn build_active_model(d: Tenant) -> ActiveModel {
        ActiveModel {
            id: match d.id {
                Some(id) => Set(id),
                None => NotSet,
            },
            uuid: match d.uuid {
                Some(uuid) => Set(string_to_bytes(&uuid)),
                None => NotSet,
            },
            business_name: Set(d.business_name),
            company_name: Set(d.company_name),
            tax_id: Set(d.tax_id),
            email: Set(d.email),
            phone: Set(d.phone),
            website: Set(d.website),
            address_line1: Set(d.address_line1),
            address_line2: Set(d.address_line2),
            locality: Set(d.locality),
            administrative_area: Set(d.administrative_area),
            postal_code: Set(d.postal_code),
            country_code: Set(d.country_code),
            payment_grace_days: match d.payment_grace_days {
                Some(days) => Set(days),
                None => NotSet,
            },
            created_at: NotSet,
            created_by: match d.created_by {
                Some(cb) => Set(Some(cb)),
                None => NotSet,
            },
            updated_at: NotSet,
            updated_by: match d.updated_by {
                Some(ub) => Set(Some(ub)),
                None => NotSet,
            },
        }
    }

    fn from_model(e: Model) -> Tenant {
        Tenant {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            business_name: e.business_name,
            company_name: e.company_name,
            tax_id: e.tax_id,
            email: e.email,
            phone: e.phone,
            website: e.website,
            address_line1: e.address_line1,
            address_line2: e.address_line2,
            locality: e.locality,
            administrative_area: e.administrative_area,
            postal_code: e.postal_code,
            country_code: e.country_code,
            payment_grace_days: Some(e.payment_grace_days),
            created_by: e.created_by,
            updated_by: e.updated_by,
            created_at: Some(e.created_at.naive_utc()),
            updated_at: Some(e.updated_at.naive_utc()),
        }
    }

    fn from_active_model(mut e: ActiveModel) -> Tenant {
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => Tenant {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                business_name: e.business_name.unwrap(),
                company_name: e.company_name.take().flatten(),
                tax_id: e.tax_id.unwrap(),
                email: e.email.take().flatten(),
                phone: e.phone.take().flatten(),
                website: e.website.take().flatten(),
                address_line1: e.address_line1.take().flatten(),
                address_line2: e.address_line2.take().flatten(),
                locality: e.locality.take().flatten(),
                administrative_area: e.administrative_area.take().flatten(),
                postal_code: e.postal_code.take().flatten(),
                country_code: e.country_code.take().flatten(),
                payment_grace_days: e.payment_grace_days.take(),
                created_by: e.created_by.take().flatten(),
                updated_by: e.updated_by.take().flatten(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
            },
        }
    }
}
