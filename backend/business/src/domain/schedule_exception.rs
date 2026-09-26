use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::ScheduleExceptionType;
use chrono::NaiveDateTime;
use entity::schedule_exception_entity::{ActiveModel, Model};
use sea_orm::prelude::Date;
use sea_orm::{NotSet, Set};
use std::str::FromStr;

/// `EPIC-SC-02-S04` (`HRMS-606`): a day exception against a demand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleException {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub demand_id: i64,
    pub date: Date,
    pub exception_type: ScheduleExceptionType,
    pub new_driver_id: Option<i64>,
    pub new_vehicle_id: Option<i64>,
    pub reason: Option<String>,
    /// `EPIC-SC-03`'s trip id, once that domain exists. No referential
    /// integrity is enforced yet (see the entity's own doc comment).
    pub extra_trip_id: Option<i64>,
    pub status: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct ScheduleExceptionEntityMapper {}

impl EntityMapper<ScheduleException, Model, ActiveModel> for ScheduleExceptionEntityMapper {
    fn build_active_model(d: ScheduleException) -> ActiveModel {
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
            date: Set(d.date),
            exception_type: Set(d.exception_type.to_string()),
            new_driver_id: Set(d.new_driver_id),
            new_vehicle_id: Set(d.new_vehicle_id),
            reason: Set(d.reason),
            extra_trip_id: Set(d.extra_trip_id),
            status: Set(d.status),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> ScheduleException {
        ScheduleException {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            demand_id: e.demand_id,
            date: e.date,
            // A stored value the vocabulary no longer knows degrades to
            // `Cancellation` rather than panicking a read -- the same
            // shape `VehicleStatus::from_str(...).unwrap_or_default()`
            // uses; writes cannot create one (`reject_unknown_schedule_
            // exception_type` refuses it first).
            exception_type: ScheduleExceptionType::from_str(&e.exception_type)
                .unwrap_or(ScheduleExceptionType::Cancellation),
            new_driver_id: e.new_driver_id,
            new_vehicle_id: e.new_vehicle_id,
            reason: e.reason,
            extra_trip_id: e.extra_trip_id,
            status: e.status,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> ScheduleException {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => ScheduleException {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                demand_id: e.demand_id.take().unwrap_or_default(),
                date: e.date.take().unwrap_or_default(),
                exception_type: e
                    .exception_type
                    .take()
                    .and_then(|value| ScheduleExceptionType::from_str(&value).ok())
                    .unwrap_or(ScheduleExceptionType::Cancellation),
                new_driver_id: e.new_driver_id.take().flatten(),
                new_vehicle_id: e.new_vehicle_id.take().flatten(),
                reason: e.reason.take().flatten(),
                extra_trip_id: e.extra_trip_id.take().flatten(),
                status: e.status.take().flatten(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
