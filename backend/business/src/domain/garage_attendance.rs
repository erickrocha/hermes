use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::{GarageAttendanceOrigin, GarageAttendanceStatus};
use chrono::{NaiveDate, NaiveDateTime};
use entity::garage_attendance_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};
use std::str::FromStr;

/// `EPIC-GA-02-S01` (`HRMS-958`): a triage -- the record that a vehicle is at base for services (`TRM-410`).
#[derive(Debug, Clone, PartialEq)]
pub struct GarageAttendance {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub attendance_date: NaiveDate,
    pub checked_in_at: NaiveDateTime,
    pub status: GarageAttendanceStatus,
    pub manual_priority: Option<i32>,
    pub released_at: Option<NaiveDateTime>,
    pub origin: GarageAttendanceOrigin,
    pub active_marker: Option<i32>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct GarageAttendanceEntityMapper {}

impl EntityMapper<GarageAttendance, Model, ActiveModel> for GarageAttendanceEntityMapper {
    fn build_active_model(d: GarageAttendance) -> ActiveModel {
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
            attendance_date: Set(d.attendance_date),
            checked_in_at: Set(d.checked_in_at.and_utc()),
            status: Set(d.status.to_string()),
            manual_priority: Set(d.manual_priority),
            released_at: Set(d.released_at.map(|dt| dt.and_utc())),
            origin: Set(d.origin.to_string()),
            active_marker: Set(d.active_marker),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> GarageAttendance {
        GarageAttendance {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            vehicle_id: e.vehicle_id,
            attendance_date: e.attendance_date,
            checked_in_at: e.checked_in_at.naive_utc(),
            status: GarageAttendanceStatus::from_str(&e.status).unwrap_or_default(),
            manual_priority: e.manual_priority,
            released_at: e.released_at.map(|dt| dt.naive_utc()),
            origin: GarageAttendanceOrigin::from_str(&e.origin).unwrap_or_default(),
            active_marker: e.active_marker,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> GarageAttendance {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => GarageAttendance {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                vehicle_id: e.vehicle_id.take().unwrap_or_default(),
                attendance_date: e.attendance_date.take().unwrap_or_default(),
                checked_in_at: e.checked_in_at.take().map(|dt| dt.naive_utc()).unwrap_or_default(),
                status: e.status.take().and_then(|v| GarageAttendanceStatus::from_str(&v).ok()).unwrap_or_default(),
                manual_priority: e.manual_priority.take().flatten(),
                released_at: e.released_at.take().flatten().map(|dt| dt.naive_utc()),
                origin: e.origin.take().and_then(|v| GarageAttendanceOrigin::from_str(&v).ok()).unwrap_or_default(),
                active_marker: e.active_marker.take().flatten(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
