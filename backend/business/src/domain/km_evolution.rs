use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::KmOrigin;
use chrono::{NaiveDateTime, Utc};
use entity::km_evolution_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};
use std::str::FromStr;

/// `EPIC-CK-01-S01` (`HRMS-650`): one official odometer reading.
#[derive(Debug, Clone, PartialEq)]
pub struct KmEvolution {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub km: f64,
    pub recorded_at: Option<NaiveDateTime>,
    pub origin: KmOrigin,
    pub source_entity: Option<String>,
    pub source_entity_id: Option<i64>,
    pub notes: Option<String>,
    pub recorded_by_user_id: Option<i64>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct KmEvolutionEntityMapper {}

impl EntityMapper<KmEvolution, Model, ActiveModel> for KmEvolutionEntityMapper {
    fn build_active_model(d: KmEvolution) -> ActiveModel {
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
            km: Set(d.km),
            // A caller states when the reading was taken; absent means now.
            recorded_at: Set(d
                .recorded_at
                .map(|dt| dt.and_utc())
                .unwrap_or_else(Utc::now)),
            origin: Set(d.origin.to_string()),
            source_entity: Set(d.source_entity),
            source_entity_id: Set(d.source_entity_id),
            notes: Set(d.notes),
            recorded_by_user_id: Set(d.recorded_by_user_id),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> KmEvolution {
        KmEvolution {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            vehicle_id: e.vehicle_id,
            km: e.km,
            recorded_at: Some(e.recorded_at.naive_utc()),
            origin: KmOrigin::from_str(&e.origin).unwrap_or(KmOrigin::Manual),
            source_entity: e.source_entity,
            source_entity_id: e.source_entity_id,
            notes: e.notes,
            recorded_by_user_id: e.recorded_by_user_id,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> KmEvolution {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => KmEvolution {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                vehicle_id: e.vehicle_id.take().unwrap_or_default(),
                km: e.km.take().unwrap_or_default(),
                recorded_at: e.recorded_at.take().map(|dt| dt.naive_utc()),
                origin: e
                    .origin
                    .take()
                    .and_then(|value| KmOrigin::from_str(&value).ok())
                    .unwrap_or(KmOrigin::Manual),
                source_entity: e.source_entity.take().flatten(),
                source_entity_id: e.source_entity_id.take().flatten(),
                notes: e.notes.take().flatten(),
                recorded_by_user_id: e.recorded_by_user_id.take().flatten(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
