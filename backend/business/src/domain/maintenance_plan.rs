use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::MaintenancePlanStatus;
use chrono::{NaiveDate, NaiveDateTime, Utc};
use entity::maintenance_plan_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};
use std::str::FromStr;

/// `EPIC-MT-03-S01` (`HRMS-703`): a planned maintenance window for a
/// vehicle. The work orders it covers live on their own side
/// (`work_order.maintenance_plan_id`), not as a field here.
#[derive(Debug, Clone, PartialEq)]
pub struct MaintenancePlan {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub date: NaiveDate,
    pub planned_start: Option<NaiveDateTime>,
    pub planned_end: Option<NaiveDateTime>,
    pub status: MaintenancePlanStatus,
    pub affects_schedule: bool,
    pub origin: String,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct MaintenancePlanEntityMapper {}

impl EntityMapper<MaintenancePlan, Model, ActiveModel> for MaintenancePlanEntityMapper {
    fn build_active_model(d: MaintenancePlan) -> ActiveModel {
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
            date: Set(d.date),
            planned_start: Set(d.planned_start.map(|dt| dt.and_utc())),
            planned_end: Set(d.planned_end.map(|dt| dt.and_utc())),
            status: Set(d.status.to_string()),
            affects_schedule: Set(d.affects_schedule),
            origin: Set(d.origin),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> MaintenancePlan {
        MaintenancePlan {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            vehicle_id: e.vehicle_id,
            date: e.date,
            planned_start: e.planned_start.map(|dt| dt.naive_utc()),
            planned_end: e.planned_end.map(|dt| dt.naive_utc()),
            status: MaintenancePlanStatus::from_str(&e.status).unwrap_or(MaintenancePlanStatus::Scheduled),
            affects_schedule: e.affects_schedule,
            origin: e.origin,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> MaintenancePlan {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => MaintenancePlan {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                vehicle_id: e.vehicle_id.take().unwrap_or_default(),
                date: e.date.take().unwrap_or_else(|| Utc::now().date_naive()),
                planned_start: e.planned_start.take().flatten().map(|dt| dt.naive_utc()),
                planned_end: e.planned_end.take().flatten().map(|dt| dt.naive_utc()),
                status: e
                    .status
                    .take()
                    .and_then(|value| MaintenancePlanStatus::from_str(&value).ok())
                    .unwrap_or(MaintenancePlanStatus::Scheduled),
                affects_schedule: e.affects_schedule.take().unwrap_or_default(),
                origin: e.origin.take().unwrap_or_default(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
