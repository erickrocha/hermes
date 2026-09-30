use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::NaiveDateTime;
use entity::garage_call_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};

/// `EPIC-GA-05-S02` (`HRMS-963`): a manual call of a vehicle to base (`TRM-483`).
#[derive(Debug, Clone, PartialEq)]
pub struct GarageCall {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub called_at: NaiveDateTime,
    pub called_by_user_id: Option<i64>,
    pub cancelled_at: Option<NaiveDateTime>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct GarageCallEntityMapper {}

impl EntityMapper<GarageCall, Model, ActiveModel> for GarageCallEntityMapper {
    fn build_active_model(d: GarageCall) -> ActiveModel {
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
            called_at: Set(d.called_at.and_utc()),
            called_by_user_id: Set(d.called_by_user_id),
            cancelled_at: Set(d.cancelled_at.map(|dt| dt.and_utc())),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> GarageCall {
        GarageCall {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            vehicle_id: e.vehicle_id,
            called_at: e.called_at.naive_utc(),
            called_by_user_id: e.called_by_user_id,
            cancelled_at: e.cancelled_at.map(|dt| dt.naive_utc()),
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> GarageCall {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => GarageCall {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                vehicle_id: e.vehicle_id.take().unwrap_or_default(),
                called_at: e.called_at.take().map(|dt| dt.naive_utc()).unwrap_or_default(),
                called_by_user_id: e.called_by_user_id.take().flatten(),
                cancelled_at: e.cancelled_at.take().flatten().map(|dt| dt.naive_utc()),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
