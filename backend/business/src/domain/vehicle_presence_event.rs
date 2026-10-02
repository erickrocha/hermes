use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::PresenceEventKind;
use chrono::NaiveDateTime;
use entity::vehicle_presence_event_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};
use std::str::FromStr;

/// `EPIC-GA-03-S01` (`HRMS-960`): one physical arrival or departure.
#[derive(Debug, Clone, PartialEq)]
pub struct VehiclePresenceEvent {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub kind: PresenceEventKind,
    pub occurred_at: NaiveDateTime,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct VehiclePresenceEventEntityMapper {}

impl EntityMapper<VehiclePresenceEvent, Model, ActiveModel> for VehiclePresenceEventEntityMapper {
    fn build_active_model(d: VehiclePresenceEvent) -> ActiveModel {
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
            vehicle_id: Set(d.vehicle_id),
            kind: Set(d.kind.to_string()),
            occurred_at: Set(d.occurred_at.and_utc()),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> VehiclePresenceEvent {
        VehiclePresenceEvent {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            vehicle_id: e.vehicle_id,
            kind: PresenceEventKind::from_str(&e.kind).unwrap_or_default(),
            occurred_at: e.occurred_at.naive_utc(),
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> VehiclePresenceEvent {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => VehiclePresenceEvent {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                vehicle_id: e.vehicle_id.take().unwrap_or_default(),
                kind: e.kind.take().and_then(|v| PresenceEventKind::from_str(&v).ok()).unwrap_or_default(),
                occurred_at: e.occurred_at.take().map(|dt| dt.naive_utc()).unwrap_or_default(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
