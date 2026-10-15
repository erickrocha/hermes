use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::{NaiveDate, NaiveDateTime};
use entity::preventive_plan_extension_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};

/// `EPIC-MT-07-S05` (`HRMS-710`): one auditable extension of a preventive
/// plan after a technical inspection (`TRM-312…320`).
#[derive(Debug, Clone, PartialEq)]
pub struct PreventivePlanExtension {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub preventive_plan_id: i64,
    pub work_order_item_id: i64,
    pub inspection_km: f64,
    pub granted_km: f64,
    pub resulting_limit_km: f64,
    pub description: String,
    pub previous_last_service_km: Option<f64>,
    pub previous_last_service_date: Option<NaiveDate>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct PreventivePlanExtensionEntityMapper {}

impl EntityMapper<PreventivePlanExtension, Model, ActiveModel> for PreventivePlanExtensionEntityMapper {
    fn build_active_model(d: PreventivePlanExtension) -> ActiveModel {
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
            work_order_item_id: Set(d.work_order_item_id),
            inspection_km: Set(d.inspection_km),
            granted_km: Set(d.granted_km),
            resulting_limit_km: Set(d.resulting_limit_km),
            description: Set(d.description),
            previous_last_service_km: Set(d.previous_last_service_km),
            previous_last_service_date: Set(d.previous_last_service_date),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> PreventivePlanExtension {
        PreventivePlanExtension {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            preventive_plan_id: e.preventive_plan_id,
            work_order_item_id: e.work_order_item_id,
            inspection_km: e.inspection_km,
            granted_km: e.granted_km,
            resulting_limit_km: e.resulting_limit_km,
            description: e.description,
            previous_last_service_km: e.previous_last_service_km,
            previous_last_service_date: e.previous_last_service_date,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> PreventivePlanExtension {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => PreventivePlanExtension {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                preventive_plan_id: e.preventive_plan_id.take().unwrap_or_default(),
                work_order_item_id: e.work_order_item_id.take().unwrap_or_default(),
                inspection_km: e.inspection_km.take().unwrap_or_default(),
                granted_km: e.granted_km.take().unwrap_or_default(),
                resulting_limit_km: e.resulting_limit_km.take().unwrap_or_default(),
                description: e.description.take().unwrap_or_default(),
                previous_last_service_km: e.previous_last_service_km.take().flatten(),
                previous_last_service_date: e.previous_last_service_date.take().flatten(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
