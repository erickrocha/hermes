use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::Role;
use chrono::NaiveDateTime;
use entity::access_profile_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};
use std::str::FromStr;

/// `C-032` (`HRMS-851`): a tenant's access profile -- a named set of permission codes for one role
/// below the owner. The codes are validated against `domain::permission` when saved.
#[derive(Debug, Clone, PartialEq)]
pub struct AccessProfile {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub name: String,
    pub name_key: String,
    pub base_role: Role,
    pub active: bool,
    pub permissions: Vec<String>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct AccessProfileEntityMapper {}

fn parse_permissions(text: &str) -> Vec<String> {
    serde_json::from_str(text).unwrap_or_default()
}

impl EntityMapper<AccessProfile, Model, ActiveModel> for AccessProfileEntityMapper {
    fn build_active_model(d: AccessProfile) -> ActiveModel {
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
            name: Set(d.name),
            name_key: Set(d.name_key),
            base_role: Set(d.base_role.to_string()),
            active: Set(d.active),
            permissions: Set(serde_json::to_string(&d.permissions).unwrap_or_else(|_| "[]".into())),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> AccessProfile {
        AccessProfile {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            name: e.name,
            name_key: e.name_key,
            base_role: Role::from_str(&e.base_role).unwrap_or(Role::TenantUser),
            active: e.active,
            permissions: parse_permissions(&e.permissions),
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> AccessProfile {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => AccessProfile {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                name: e.name.take().unwrap_or_default(),
                name_key: e.name_key.take().unwrap_or_default(),
                base_role: e.base_role.take().and_then(|r| Role::from_str(&r).ok()).unwrap_or(Role::TenantUser),
                active: e.active.take().unwrap_or(true),
                permissions: e.permissions.take().map(|p| parse_permissions(&p)).unwrap_or_default(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
