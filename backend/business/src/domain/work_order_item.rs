use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::WorkOrderItemStatus;
use chrono::NaiveDateTime;
use entity::work_order_item_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};
use std::str::FromStr;

/// `EPIC-MT-01-S02` (`HRMS-701`): one pendency of a `WorkOrder`.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkOrderItem {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub work_order_id: i64,
    pub description: String,
    pub item_type: Option<String>,
    pub status: WorkOrderItemStatus,
    pub observation: Option<String>,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<NaiveDateTime>,
    pub resolution_description: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct WorkOrderItemEntityMapper {}

impl EntityMapper<WorkOrderItem, Model, ActiveModel> for WorkOrderItemEntityMapper {
    fn build_active_model(d: WorkOrderItem) -> ActiveModel {
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
            work_order_id: Set(d.work_order_id),
            description: Set(d.description),
            item_type: Set(d.item_type),
            status: Set(d.status.to_string()),
            observation: Set(d.observation),
            resolved_by: Set(d.resolved_by),
            resolved_at: Set(d.resolved_at.map(|dt| dt.and_utc())),
            resolution_description: Set(d.resolution_description),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> WorkOrderItem {
        WorkOrderItem {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            work_order_id: e.work_order_id,
            description: e.description,
            item_type: e.item_type,
            status: WorkOrderItemStatus::from_str(&e.status).unwrap_or(WorkOrderItemStatus::Pending),
            observation: e.observation,
            resolved_by: e.resolved_by,
            resolved_at: e.resolved_at.map(|dt| dt.naive_utc()),
            resolution_description: e.resolution_description,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> WorkOrderItem {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => WorkOrderItem {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                work_order_id: e.work_order_id.take().unwrap_or_default(),
                description: e.description.take().unwrap_or_default(),
                item_type: e.item_type.take().flatten(),
                status: e
                    .status
                    .take()
                    .and_then(|value| WorkOrderItemStatus::from_str(&value).ok())
                    .unwrap_or(WorkOrderItemStatus::Pending),
                observation: e.observation.take().flatten(),
                resolved_by: e.resolved_by.take().flatten(),
                resolved_at: e.resolved_at.take().flatten().map(|dt| dt.naive_utc()),
                resolution_description: e.resolution_description.take().flatten(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
