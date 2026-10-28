use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::NaiveDateTime;
use entity::customer_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};

/// `EPIC-SC-01-S01` (`HRMS-600`): a customer is a name and a status, owned by exactly one tenant.
/// `EPIC-SC-01-S04` made it a legal person: a taxpayer document, a legal name and an address in the same
/// shape the tenant's own address has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Customer {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    /// The trade name -- what the office calls the customer.
    pub name: String,
    /// The registered legal name; a person has none.
    pub legal_name: Option<String>,
    /// CPF or CNPJ, digits only once validated; unique per tenant when present.
    pub tax_id: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub locality: Option<String>,
    pub administrative_area: Option<String>,
    pub postal_code: Option<String>,
    pub country_code: Option<String>,
    pub status: String,
    pub notes: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct CustomerEntityMapper {}

impl EntityMapper<Customer, Model, ActiveModel> for CustomerEntityMapper {
    fn build_active_model(d: Customer) -> ActiveModel {
        ActiveModel {
            id: match d.id {
                Some(id) => Set(id),
                None => NotSet,
            },
            uuid: match d.uuid {
                Some(uuid) => Set(string_to_bytes(&uuid)),
                None => NotSet,
            },
            // D-06: overwritten by `enforce_tenant` with the caller's own
            // scope before the row reaches the database.
            tenant_id: Set(d.tenant_id),
            name: Set(d.name.to_owned()),
            legal_name: Set(d.legal_name),
            tax_id: Set(d.tax_id),
            address_line1: Set(d.address_line1),
            address_line2: Set(d.address_line2),
            locality: Set(d.locality),
            administrative_area: Set(d.administrative_area),
            postal_code: Set(d.postal_code),
            country_code: Set(d.country_code),
            status: Set(d.status.to_owned()),
            notes: Set(d.notes),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> Customer {
        Customer {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            name: e.name,
            legal_name: e.legal_name,
            tax_id: e.tax_id,
            address_line1: e.address_line1,
            address_line2: e.address_line2,
            locality: e.locality,
            administrative_area: e.administrative_area,
            postal_code: e.postal_code,
            country_code: e.country_code,
            status: e.status,
            notes: e.notes,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> Customer {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => Customer {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                name: e.name.take().unwrap_or_default(),
                legal_name: e.legal_name.take().flatten(),
                tax_id: e.tax_id.take().flatten(),
                address_line1: e.address_line1.take().flatten(),
                address_line2: e.address_line2.take().flatten(),
                locality: e.locality.take().flatten(),
                administrative_area: e.administrative_area.take().flatten(),
                postal_code: e.postal_code.take().flatten(),
                country_code: e.country_code.take().flatten(),
                status: e.status.take().unwrap_or_default(),
                notes: e.notes.take().flatten(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
