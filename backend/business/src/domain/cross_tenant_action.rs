use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::NaiveDateTime;
use entity::cross_tenant_action_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set, TryIntoModel};

pub const KIND_READ: &str = "read";
pub const KIND_WRITE: &str = "write";

/// `C-036` (`HRMS-877`): a request one tenant made inside a descendant -- who, from which tenant, into which, which
/// operation and record, read or write, and when. The affected tenant reads it back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossTenantAction {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub actor_user_id: i64,
    pub actor_email: String,
    pub actor_tenant_id: i64,
    pub affected_tenant_id: i64,
    pub kind: String,
    pub method: String,
    /// The matched route template (`/vehicle/uuid/{uuid}`).
    pub route: String,
    /// The concrete path, which names the record.
    pub path: String,
    pub occurred_at: NaiveDateTime,
}

impl CrossTenantAction {
    /// A safe method reads, every other writes.
    pub fn kind_of(method: &str) -> &'static str {
        if matches!(method, "GET" | "HEAD" | "OPTIONS") { KIND_READ } else { KIND_WRITE }
    }
}

pub struct CrossTenantActionEntityMapper;

impl EntityMapper<CrossTenantAction, Model, ActiveModel> for CrossTenantActionEntityMapper {
    fn build_active_model(d: CrossTenantAction) -> ActiveModel {
        ActiveModel {
            id: d.id.map_or(NotSet, Set),
            uuid: Set(d.uuid.map_or_else(|| uuid::Uuid::new_v4().as_bytes().to_vec(), |u| string_to_bytes(&u))),
            actor_user_id: Set(d.actor_user_id as i32),
            actor_email: Set(d.actor_email),
            actor_tenant_id: Set(d.actor_tenant_id as i32),
            affected_tenant_id: Set(d.affected_tenant_id as i32),
            kind: Set(d.kind),
            method: Set(d.method),
            route: Set(d.route),
            path: Set(d.path),
            occurred_at: Set(d.occurred_at.and_utc()),
        }
    }

    fn from_model(e: Model) -> CrossTenantAction {
        CrossTenantAction {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            actor_user_id: i64::from(e.actor_user_id),
            actor_email: e.actor_email,
            actor_tenant_id: i64::from(e.actor_tenant_id),
            affected_tenant_id: i64::from(e.affected_tenant_id),
            kind: e.kind,
            method: e.method,
            route: e.route,
            path: e.path,
            occurred_at: e.occurred_at.naive_utc(),
        }
    }

    fn from_active_model(e: ActiveModel) -> CrossTenantAction {
        match e.try_into_model() {
            Ok(model) => Self::from_model(model),
            Err(_) => CrossTenantAction {
                id: None,
                uuid: None,
                actor_user_id: 0,
                actor_email: String::new(),
                actor_tenant_id: 0,
                affected_tenant_id: 0,
                kind: String::new(),
                method: String::new(),
                route: String::new(),
                path: String::new(),
                occurred_at: NaiveDateTime::default(),
            },
        }
    }
}
