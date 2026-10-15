use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::{GarageServiceState};
use chrono::{NaiveDateTime};
use entity::garage_service_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};
use std::str::FromStr;

/// `EPIC-GA-02-S01` (`HRMS-958`): one service of a triage, identified by the triage and the normalised service name (`TRM-433`).
#[derive(Debug, Clone, PartialEq)]
pub struct GarageService {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub attendance_id: i64,
    pub service_model_id: i64,
    pub name_key: String,
    pub state: GarageServiceState,
    pub performed_at: Option<NaiveDateTime>,
    pub marked_at: Option<NaiveDateTime>,
    pub forced_pending_at: Option<NaiveDateTime>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct GarageServiceEntityMapper {}

impl EntityMapper<GarageService, Model, ActiveModel> for GarageServiceEntityMapper {
    fn build_active_model(d: GarageService) -> ActiveModel {
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
            attendance_id: Set(d.attendance_id),
            service_model_id: Set(d.service_model_id),
            name_key: Set(d.name_key),
            state: Set(d.state.to_string()),
            performed_at: Set(d.performed_at.map(|dt| dt.and_utc())),
            marked_at: Set(d.marked_at.map(|dt| dt.and_utc())),
            forced_pending_at: Set(d.forced_pending_at.map(|dt| dt.and_utc())),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> GarageService {
        GarageService {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            attendance_id: e.attendance_id,
            service_model_id: e.service_model_id,
            name_key: e.name_key,
            state: GarageServiceState::from_str(&e.state).unwrap_or_default(),
            performed_at: e.performed_at.map(|dt| dt.naive_utc()),
            marked_at: e.marked_at.map(|dt| dt.naive_utc()),
            forced_pending_at: e.forced_pending_at.map(|dt| dt.naive_utc()),
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> GarageService {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => GarageService {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                attendance_id: e.attendance_id.take().unwrap_or_default(),
                service_model_id: e.service_model_id.take().unwrap_or_default(),
                name_key: e.name_key.take().unwrap_or_default(),
                state: e.state.take().and_then(|v| GarageServiceState::from_str(&v).ok()).unwrap_or_default(),
                performed_at: e.performed_at.take().flatten().map(|dt| dt.naive_utc()),
                marked_at: e.marked_at.take().flatten().map(|dt| dt.naive_utc()),
                forced_pending_at: e.forced_pending_at.take().flatten().map(|dt| dt.naive_utc()),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
