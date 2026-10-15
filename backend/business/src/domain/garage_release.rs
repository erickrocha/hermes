use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::{GarageServiceState, WorkOrderItemStatus};
use chrono::NaiveDateTime;
use entity::garage_release_entity::{ActiveModel as ReleaseActiveModel, Model as ReleaseModel};
use entity::garage_release_item_entity::{ActiveModel as ItemActiveModel, Model as ItemModel};
use sea_orm::{NotSet, Set};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub struct GarageRelease {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub attendance_id: i64,
    pub vehicle_id: i64,
    pub work_order_id: i64,
    pub justification: String,
    pub responsible_user_id: i64,
    pub released_at: NaiveDateTime,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GarageReleaseItem {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub garage_release_id: i64,
    pub garage_service_id: i64,
    pub work_order_item_id: i64,
    pub service_name: String,
    pub garage_state: GarageServiceState,
    pub work_order_status: WorkOrderItemStatus,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct GarageReleaseEntityMapper;

impl EntityMapper<GarageRelease, ReleaseModel, ReleaseActiveModel> for GarageReleaseEntityMapper {
    fn build_active_model(domain: GarageRelease) -> ReleaseActiveModel {
        ReleaseActiveModel {
            id: domain.id.map_or(NotSet, Set),
            uuid: domain
                .uuid
                .map_or(NotSet, |uuid| Set(string_to_bytes(&uuid))),
            tenant_id: Set(domain.tenant_id),
            attendance_id: Set(domain.attendance_id),
            vehicle_id: Set(domain.vehicle_id),
            work_order_id: Set(domain.work_order_id),
            justification: Set(domain.justification),
            responsible_user_id: Set(domain.responsible_user_id),
            released_at: Set(domain.released_at.and_utc()),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(model: ReleaseModel) -> GarageRelease {
        GarageRelease {
            id: Some(model.id),
            uuid: Some(bytes_para_string(model.uuid)),
            tenant_id: model.tenant_id,
            attendance_id: model.attendance_id,
            vehicle_id: model.vehicle_id,
            work_order_id: model.work_order_id,
            justification: model.justification,
            responsible_user_id: model.responsible_user_id,
            released_at: model.released_at.naive_utc(),
            created_at: Some(model.created_at.naive_utc()),
            created_by: model.created_by,
            updated_at: Some(model.updated_at.naive_utc()),
            updated_by: model.updated_by,
        }
    }

    fn from_active_model(mut active: ReleaseActiveModel) -> GarageRelease {
        use sea_orm::TryIntoModel;
        match active.clone().try_into_model() {
            Ok(model) => Self::from_model(model),
            Err(_) => GarageRelease {
                id: active.id.take(),
                uuid: active.uuid.take().map(bytes_para_string),
                tenant_id: active.tenant_id.take().flatten(),
                attendance_id: active.attendance_id.take().unwrap_or_default(),
                vehicle_id: active.vehicle_id.take().unwrap_or_default(),
                work_order_id: active.work_order_id.take().unwrap_or_default(),
                justification: active.justification.take().unwrap_or_default(),
                responsible_user_id: active.responsible_user_id.take().unwrap_or_default(),
                released_at: active
                    .released_at
                    .take()
                    .map(|at| at.naive_utc())
                    .unwrap_or_default(),
                created_at: active.created_at.take().map(|at| at.naive_utc()),
                created_by: active.created_by.take().flatten(),
                updated_at: active.updated_at.take().map(|at| at.naive_utc()),
                updated_by: active.updated_by.take().flatten(),
            },
        }
    }
}

pub struct GarageReleaseItemEntityMapper;

impl EntityMapper<GarageReleaseItem, ItemModel, ItemActiveModel> for GarageReleaseItemEntityMapper {
    fn build_active_model(domain: GarageReleaseItem) -> ItemActiveModel {
        ItemActiveModel {
            id: domain.id.map_or(NotSet, Set),
            uuid: domain
                .uuid
                .map_or(NotSet, |uuid| Set(string_to_bytes(&uuid))),
            tenant_id: Set(domain.tenant_id),
            garage_release_id: Set(domain.garage_release_id),
            garage_service_id: Set(domain.garage_service_id),
            work_order_item_id: Set(domain.work_order_item_id),
            service_name: Set(domain.service_name),
            garage_state: Set(domain.garage_state.to_string()),
            work_order_status: Set(domain.work_order_status.to_string()),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(model: ItemModel) -> GarageReleaseItem {
        GarageReleaseItem {
            id: Some(model.id),
            uuid: Some(bytes_para_string(model.uuid)),
            tenant_id: model.tenant_id,
            garage_release_id: model.garage_release_id,
            garage_service_id: model.garage_service_id,
            work_order_item_id: model.work_order_item_id,
            service_name: model.service_name,
            garage_state: GarageServiceState::from_str(&model.garage_state).unwrap_or_default(),
            work_order_status: WorkOrderItemStatus::from_str(&model.work_order_status)
                .unwrap_or_default(),
            created_at: Some(model.created_at.naive_utc()),
            created_by: model.created_by,
            updated_at: Some(model.updated_at.naive_utc()),
            updated_by: model.updated_by,
        }
    }

    fn from_active_model(mut active: ItemActiveModel) -> GarageReleaseItem {
        use sea_orm::TryIntoModel;
        match active.clone().try_into_model() {
            Ok(model) => Self::from_model(model),
            Err(_) => GarageReleaseItem {
                id: active.id.take(),
                uuid: active.uuid.take().map(bytes_para_string),
                tenant_id: active.tenant_id.take().flatten(),
                garage_release_id: active.garage_release_id.take().unwrap_or_default(),
                garage_service_id: active.garage_service_id.take().unwrap_or_default(),
                work_order_item_id: active.work_order_item_id.take().unwrap_or_default(),
                service_name: active.service_name.take().unwrap_or_default(),
                garage_state: active
                    .garage_state
                    .take()
                    .and_then(|state| GarageServiceState::from_str(&state).ok())
                    .unwrap_or_default(),
                work_order_status: active
                    .work_order_status
                    .take()
                    .and_then(|status| WorkOrderItemStatus::from_str(&status).ok())
                    .unwrap_or_default(),
                created_at: active.created_at.take().map(|at| at.naive_utc()),
                created_by: active.created_by.take().flatten(),
                updated_at: active.updated_at.take().map(|at| at.naive_utc()),
                updated_by: active.updated_by.take().flatten(),
            },
        }
    }
}
