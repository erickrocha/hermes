use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::NaiveDateTime;
use entity::preventive_plan_alert_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};

/// `EPIC-MT-07-S08` (`HRMS-713`): an intermediate inspection alert declared
/// on a preventive plan (`TRM-323…325`).
#[derive(Debug, Clone, PartialEq)]
pub struct PreventivePlanAlert {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub preventive_plan_id: i64,
    pub at_km: f64,
    pub title: String,
    pub inspection_model: String,
    /// `TRM-325`: the plan's cycle key when the alert was discharged.
    pub discharged_cycle: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct PreventivePlanAlertEntityMapper {}

impl EntityMapper<PreventivePlanAlert, Model, ActiveModel> for PreventivePlanAlertEntityMapper {
    fn build_active_model(d: PreventivePlanAlert) -> ActiveModel {
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
            preventive_plan_id: Set(d.preventive_plan_id),
            at_km: Set(d.at_km),
            title: Set(d.title),
            inspection_model: Set(d.inspection_model),
            discharged_cycle: Set(d.discharged_cycle),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> PreventivePlanAlert {
        PreventivePlanAlert {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            preventive_plan_id: e.preventive_plan_id,
            at_km: e.at_km,
            title: e.title,
            inspection_model: e.inspection_model,
            discharged_cycle: e.discharged_cycle,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> PreventivePlanAlert {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => PreventivePlanAlert {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                preventive_plan_id: e.preventive_plan_id.take().unwrap_or_default(),
                at_km: e.at_km.take().unwrap_or_default(),
                title: e.title.take().unwrap_or_default(),
                inspection_model: e.inspection_model.take().unwrap_or_default(),
                discharged_cycle: e.discharged_cycle.take().flatten(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
