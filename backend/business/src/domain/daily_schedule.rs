use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::NaiveDateTime;
use entity::daily_schedule_entity::{ActiveModel, Model};
use sea_orm::prelude::{Date, Time};
use sea_orm::{NotSet, Set};

/// `EPIC-SC-02-S03` (`HRMS-605`): a manual one-day schedule entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailySchedule {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub demand_id: i64,
    pub driver_id: i64,
    pub vehicle_id: i64,
    pub date: Date,
    pub start_time: Option<Time>,
    pub end_time: Option<Time>,
    pub notes: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct DailyScheduleEntityMapper {}

impl EntityMapper<DailySchedule, Model, ActiveModel> for DailyScheduleEntityMapper {
    fn build_active_model(d: DailySchedule) -> ActiveModel {
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
            demand_id: Set(d.demand_id),
            driver_id: Set(d.driver_id),
            vehicle_id: Set(d.vehicle_id),
            date: Set(d.date),
            start_time: Set(d.start_time),
            end_time: Set(d.end_time),
            notes: Set(d.notes),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> DailySchedule {
        DailySchedule {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            demand_id: e.demand_id,
            driver_id: e.driver_id,
            vehicle_id: e.vehicle_id,
            date: e.date,
            start_time: e.start_time,
            end_time: e.end_time,
            notes: e.notes,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> DailySchedule {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => DailySchedule {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                demand_id: e.demand_id.take().unwrap_or_default(),
                driver_id: e.driver_id.take().unwrap_or_default(),
                vehicle_id: e.vehicle_id.take().unwrap_or_default(),
                date: e.date.take().unwrap_or_default(),
                start_time: e.start_time.take().flatten(),
                end_time: e.end_time.take().flatten(),
                notes: e.notes.take().flatten(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
