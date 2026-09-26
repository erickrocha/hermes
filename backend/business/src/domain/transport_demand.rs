use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::NaiveDateTime;
use entity::transport_demand_entity::{ActiveModel, Model};
use sea_orm::prelude::{Date, Time};
use sea_orm::{NotSet, Set};

/// `EPIC-SC-02-S01` (`HRMS-603`): a unit of recurring transport demand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportDemand {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub demand_type: String,
    pub customer_id: Option<i64>,
    pub line_name: Option<String>,
    pub shift_start: Option<Time>,
    pub shift_end: Option<Time>,
    pub days_of_week: Option<String>,
    pub specific_date: Option<Date>,
    pub priority: Option<i32>,
    pub preferred_vehicle_type: Option<String>,
    pub preferred_vehicle_model: Option<String>,
    pub specific_driver_id: Option<i64>,
    pub specific_vehicle_id: Option<i64>,
    pub active: bool,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct TransportDemandEntityMapper {}

impl EntityMapper<TransportDemand, Model, ActiveModel> for TransportDemandEntityMapper {
    fn build_active_model(d: TransportDemand) -> ActiveModel {
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
            demand_type: Set(d.demand_type),
            customer_id: Set(d.customer_id),
            line_name: Set(d.line_name),
            shift_start: Set(d.shift_start),
            shift_end: Set(d.shift_end),
            days_of_week: Set(d.days_of_week),
            specific_date: Set(d.specific_date),
            priority: Set(d.priority),
            preferred_vehicle_type: Set(d.preferred_vehicle_type),
            preferred_vehicle_model: Set(d.preferred_vehicle_model),
            specific_driver_id: Set(d.specific_driver_id),
            specific_vehicle_id: Set(d.specific_vehicle_id),
            active: Set(d.active),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> TransportDemand {
        TransportDemand {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            demand_type: e.demand_type,
            customer_id: e.customer_id,
            line_name: e.line_name,
            shift_start: e.shift_start,
            shift_end: e.shift_end,
            days_of_week: e.days_of_week,
            specific_date: e.specific_date,
            priority: e.priority,
            preferred_vehicle_type: e.preferred_vehicle_type,
            preferred_vehicle_model: e.preferred_vehicle_model,
            specific_driver_id: e.specific_driver_id,
            specific_vehicle_id: e.specific_vehicle_id,
            active: e.active,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> TransportDemand {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => TransportDemand {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                demand_type: e.demand_type.take().unwrap_or_default(),
                customer_id: e.customer_id.take().flatten(),
                line_name: e.line_name.take().flatten(),
                shift_start: e.shift_start.take().flatten(),
                shift_end: e.shift_end.take().flatten(),
                days_of_week: e.days_of_week.take().flatten(),
                specific_date: e.specific_date.take().flatten(),
                priority: e.priority.take().flatten(),
                preferred_vehicle_type: e.preferred_vehicle_type.take().flatten(),
                preferred_vehicle_model: e.preferred_vehicle_model.take().flatten(),
                specific_driver_id: e.specific_driver_id.take().flatten(),
                specific_vehicle_id: e.specific_vehicle_id.take().flatten(),
                active: e.active.take().unwrap_or(true),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
