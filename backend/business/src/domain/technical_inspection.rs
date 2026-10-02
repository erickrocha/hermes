use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::{NaiveDate, NaiveDateTime};
use entity::technical_inspection_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};

/// `EPIC-MT-07-S09` (`HRMS-714`): a technical inspection of a vehicle
/// (`TRM-333`).
#[derive(Debug, Clone, PartialEq)]
pub struct TechnicalInspection {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub inspection_model: String,
    pub inspected_at: NaiveDate,
    pub odometer_km: f64,
    pub observation: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct TechnicalInspectionEntityMapper {}

impl EntityMapper<TechnicalInspection, Model, ActiveModel> for TechnicalInspectionEntityMapper {
    fn build_active_model(d: TechnicalInspection) -> ActiveModel {
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
            inspection_model: Set(d.inspection_model),
            inspected_at: Set(d.inspected_at),
            odometer_km: Set(d.odometer_km),
            observation: Set(d.observation),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> TechnicalInspection {
        TechnicalInspection {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            vehicle_id: e.vehicle_id,
            inspection_model: e.inspection_model,
            inspected_at: e.inspected_at,
            odometer_km: e.odometer_km,
            observation: e.observation,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> TechnicalInspection {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => TechnicalInspection {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                vehicle_id: e.vehicle_id.take().unwrap_or_default(),
                inspection_model: e.inspection_model.take().unwrap_or_default(),
                inspected_at: e.inspected_at.take().unwrap_or_default(),
                odometer_km: e.odometer_km.take().unwrap_or_default(),
                observation: e.observation.take().flatten(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
