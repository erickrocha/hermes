use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::PreventiveControlType;
use chrono::{NaiveDate, NaiveDateTime};
use entity::preventive_plan_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};
use std::str::FromStr;

/// `EPIC-MT-07-S01` (`HRMS-706`): a vehicle's preventive-maintenance plan --
/// `entity-inventory.md` §5 "Preventivas" (`TRM-300…305`).
#[derive(Debug, Clone, PartialEq)]
pub struct PreventivePlan {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub plan_name: String,
    pub control_type: PreventiveControlType,
    pub interval_km: Option<f64>,
    pub interval_days: Option<i32>,
    pub last_service_km: Option<f64>,
    pub last_service_date: Option<NaiveDate>,
    /// `TRM-316`: the active extension's kilometre limit, replacing
    /// `last_service_km + interval_km` until a real service clears it.
    pub extension_limit_km: Option<f64>,
    /// `TRM-309`/`TRM-330`: the work order that last serviced the plan, or
    /// that a manual new cycle names as its origin.
    pub last_work_order_id: Option<i64>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct PreventivePlanEntityMapper {}

impl EntityMapper<PreventivePlan, Model, ActiveModel> for PreventivePlanEntityMapper {
    fn build_active_model(d: PreventivePlan) -> ActiveModel {
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
            plan_name: Set(d.plan_name),
            control_type: Set(d.control_type.to_string()),
            interval_km: Set(d.interval_km),
            interval_days: Set(d.interval_days),
            last_service_km: Set(d.last_service_km),
            last_service_date: Set(d.last_service_date),
            extension_limit_km: Set(d.extension_limit_km),
            last_work_order_id: Set(d.last_work_order_id),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> PreventivePlan {
        PreventivePlan {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            vehicle_id: e.vehicle_id,
            plan_name: e.plan_name,
            control_type: PreventiveControlType::from_str(&e.control_type).unwrap_or_default(),
            interval_km: e.interval_km,
            interval_days: e.interval_days,
            last_service_km: e.last_service_km,
            last_service_date: e.last_service_date,
            extension_limit_km: e.extension_limit_km,
            last_work_order_id: e.last_work_order_id,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> PreventivePlan {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => PreventivePlan {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                vehicle_id: e.vehicle_id.take().unwrap_or_default(),
                plan_name: e.plan_name.take().unwrap_or_default(),
                control_type: e
                    .control_type
                    .take()
                    .and_then(|v| PreventiveControlType::from_str(&v).ok())
                    .unwrap_or_default(),
                interval_km: e.interval_km.take().flatten(),
                interval_days: e.interval_days.take().flatten(),
                last_service_km: e.last_service_km.take().flatten(),
                last_service_date: e.last_service_date.take().flatten(),
                extension_limit_km: e.extension_limit_km.take().flatten(),
                last_work_order_id: e.last_work_order_id.take().flatten(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
