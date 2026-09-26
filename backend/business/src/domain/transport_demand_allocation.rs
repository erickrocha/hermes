use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::NaiveDateTime;
use entity::transport_demand_allocation_entity::{ActiveModel, Model};
use sea_orm::prelude::Date;
use sea_orm::{NotSet, Set};

/// `EPIC-SC-02-S02` (`HRMS-604`): a demand's crew over a date range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportDemandAllocation {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub demand_id: i64,
    pub driver_id: i64,
    pub vehicle_id: i64,
    pub days_of_week: Option<String>,
    pub start_date: Date,
    pub end_date: Option<Date>,
    pub active: bool,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct TransportDemandAllocationEntityMapper {}

impl EntityMapper<TransportDemandAllocation, Model, ActiveModel>
    for TransportDemandAllocationEntityMapper
{
    fn build_active_model(d: TransportDemandAllocation) -> ActiveModel {
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
            days_of_week: Set(d.days_of_week),
            start_date: Set(d.start_date),
            end_date: Set(d.end_date),
            active: Set(d.active),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> TransportDemandAllocation {
        TransportDemandAllocation {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            demand_id: e.demand_id,
            driver_id: e.driver_id,
            vehicle_id: e.vehicle_id,
            days_of_week: e.days_of_week,
            start_date: e.start_date,
            end_date: e.end_date,
            active: e.active,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> TransportDemandAllocation {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => TransportDemandAllocation {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                demand_id: e.demand_id.take().unwrap_or_default(),
                driver_id: e.driver_id.take().unwrap_or_default(),
                vehicle_id: e.vehicle_id.take().unwrap_or_default(),
                days_of_week: e.days_of_week.take().flatten(),
                start_date: e.start_date.take().unwrap_or_default(),
                end_date: e.end_date.take().flatten(),
                active: e.active.take().unwrap_or(true),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
