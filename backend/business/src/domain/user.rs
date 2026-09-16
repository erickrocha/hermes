use std::str::FromStr;
use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::NaiveDateTime;
use entity::user_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};
use crate::domain::enums::Role;

#[derive(Debug, Clone)]
pub struct User {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub email: String,
    pub name: Option<String>,
    pub password: String,
    pub enabled: bool,
    pub first_login: bool,
    pub tenant_id: Option<i64>,
    pub role: Role,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct UserEntityMapper{}
impl EntityMapper<User, Model, ActiveModel> for UserEntityMapper {
    fn build_active_model(d: User) -> ActiveModel {
        ActiveModel {
            id: match d.id {
                Some(id) => Set(id),
                None => NotSet,
            },
            uuid: match d.uuid {
                Some(uuid) => Set(string_to_bytes(&uuid)),
                None => NotSet,
            },
            name: Set(d.name.to_owned()),
            email: Set(d.email.to_owned()),
            password: Set(d.password.to_owned()),
            first_login: Set(d.first_login.to_owned()),
            enabled: Set(d.enabled.to_owned()),
            // Owned by the billing worker; a profile update must not clear it.
            blocked_reason: NotSet,
            tenant_id: Set(d.tenant_id),
            role: Set(d.role.to_string()),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: Default::default(),
        }
    }

    fn from_model(e: Model) -> User {
        User {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            name: e.name,
            email: e.email,
            password: e.password,
            enabled: e.enabled,
            first_login: e.first_login,
            tenant_id: e.tenant_id,
            role: Role::from_str(e.role.as_str()).unwrap_or(Role::TenantUser),
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> User {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => User {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                name: e.name.take().flatten(),
                email: e.email.take().unwrap_or_default(),
                password: e.password.take().unwrap_or_default(),
                enabled: e.enabled.take().unwrap_or(true),
                first_login: e.first_login.take().unwrap_or(false),
                tenant_id: e.tenant_id.take().flatten(),
                role: Role::from_str(e.role.unwrap().as_str()).unwrap_or(Role::TenantUser),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}

