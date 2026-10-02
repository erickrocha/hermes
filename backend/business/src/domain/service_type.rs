use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::NaiveDateTime;
use entity::service_type_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};

/// `EPIC-MT-05-S01` (`HRMS-704`): a tenant's service-category catalogue
/// entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ServiceType {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub code: String,
    pub name: String,
    pub category: Option<String>,
    pub active: bool,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct ServiceTypeEntityMapper {}

impl EntityMapper<ServiceType, Model, ActiveModel> for ServiceTypeEntityMapper {
    fn build_active_model(d: ServiceType) -> ActiveModel {
        ActiveModel {
            id: match d.id {
                Some(id) => Set(id),
                None => NotSet,
            },
            uuid: match d.uuid {
                Some(uuid) => Set(string_to_bytes(&uuid)),
                None => NotSet,
            },
            tenant_id: Set(d.tenant_id),
            code: Set(d.code),
            name: Set(d.name),
            category: Set(d.category),
            active: Set(d.active),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> ServiceType {
        ServiceType {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            code: e.code,
            name: e.name,
            category: e.category,
            active: e.active,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> ServiceType {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => ServiceType {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                code: e.code.take().unwrap_or_default(),
                name: e.name.take().unwrap_or_default(),
                category: e.category.take().flatten(),
                active: e.active.take().unwrap_or_default(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
