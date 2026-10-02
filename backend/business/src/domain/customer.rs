use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::NaiveDateTime;
use entity::customer_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};

/// `EPIC-SC-01-S01` (`HRMS-600`): a customer is a name and a status, owned by
/// exactly one tenant -- same shape as `Vehicle` before `C-023` widened it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Customer {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub name: String,
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
