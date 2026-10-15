use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::{GarageServiceState};
use chrono::{NaiveDateTime};
use entity::garage_service_log_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};
use std::str::FromStr;

/// `EPIC-GA-02-S02` (`HRMS-959`): the independent audit row written with every service marking (`TRM-440`).
#[derive(Debug, Clone, PartialEq)]
pub struct GarageServiceLog {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub attendance_id: i64,
    pub vehicle_id: i64,
    pub service_model_id: i64,
    pub name_key: String,
    pub new_state: GarageServiceState,
    pub acted_by_user_id: Option<i64>,
    pub acted_at: NaiveDateTime,
    pub origin: String,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct GarageServiceLogEntityMapper {}

impl EntityMapper<GarageServiceLog, Model, ActiveModel> for GarageServiceLogEntityMapper {
    fn build_active_model(d: GarageServiceLog) -> ActiveModel {
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
            vehicle_id: Set(d.vehicle_id),
            service_model_id: Set(d.service_model_id),
            name_key: Set(d.name_key),
            new_state: Set(d.new_state.to_string()),
            acted_by_user_id: Set(d.acted_by_user_id),
            acted_at: Set(d.acted_at.and_utc()),
            origin: Set(d.origin),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> GarageServiceLog {
        GarageServiceLog {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            attendance_id: e.attendance_id,
            vehicle_id: e.vehicle_id,
            service_model_id: e.service_model_id,
            name_key: e.name_key,
            new_state: GarageServiceState::from_str(&e.new_state).unwrap_or_default(),
            acted_by_user_id: e.acted_by_user_id,
            acted_at: e.acted_at.naive_utc(),
            origin: e.origin,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> GarageServiceLog {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => GarageServiceLog {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                attendance_id: e.attendance_id.take().unwrap_or_default(),
                vehicle_id: e.vehicle_id.take().unwrap_or_default(),
                service_model_id: e.service_model_id.take().unwrap_or_default(),
                name_key: e.name_key.take().unwrap_or_default(),
                new_state: e.new_state.take().and_then(|v| GarageServiceState::from_str(&v).ok()).unwrap_or_default(),
                acted_by_user_id: e.acted_by_user_id.take().flatten(),
                acted_at: e.acted_at.take().map(|dt| dt.naive_utc()).unwrap_or_default(),
                origin: e.origin.take().unwrap_or_default(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
