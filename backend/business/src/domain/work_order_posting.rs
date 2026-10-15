use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::CostSource;
use chrono::NaiveDateTime;
use entity::work_order_posting_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};
use std::str::FromStr;

/// `EPIC-MT-02-S01` (`HRMS-705`): a costed posting against a `WorkOrder` --
/// `entity-inventory.md` §4 "Lançamentos de OS" (`TRM-609`, `TRM-613…615`,
/// `TRM-688`). `work_order_item_id` is the pendency it was directed at, if
/// any (`TRM-614`).
#[derive(Debug, Clone, PartialEq)]
pub struct WorkOrderPosting {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub work_order_id: i64,
    pub work_order_item_id: Option<i64>,
    pub part_id: i64,
    pub quantity: f64,
    pub unit_value_cents: i64,
    pub total_value_cents: i64,
    pub cost_source: CostSource,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct WorkOrderPostingEntityMapper {}

impl EntityMapper<WorkOrderPosting, Model, ActiveModel> for WorkOrderPostingEntityMapper {
    fn build_active_model(d: WorkOrderPosting) -> ActiveModel {
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
            work_order_item_id: Set(d.work_order_item_id),
            part_id: Set(d.part_id),
            quantity: Set(d.quantity),
            unit_value_cents: Set(d.unit_value_cents),
            total_value_cents: Set(d.total_value_cents),
            cost_source: Set(d.cost_source.to_string()),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> WorkOrderPosting {
        WorkOrderPosting {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            work_order_id: e.work_order_id,
            work_order_item_id: e.work_order_item_id,
            part_id: e.part_id,
            quantity: e.quantity,
            unit_value_cents: e.unit_value_cents,
            total_value_cents: e.total_value_cents,
            cost_source: CostSource::from_str(&e.cost_source).unwrap_or_default(),
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> WorkOrderPosting {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => WorkOrderPosting {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                work_order_id: e.work_order_id.take().unwrap_or_default(),
                work_order_item_id: e.work_order_item_id.take().flatten(),
                part_id: e.part_id.take().unwrap_or_default(),
                quantity: e.quantity.take().unwrap_or_default(),
                unit_value_cents: e.unit_value_cents.take().unwrap_or_default(),
                total_value_cents: e.total_value_cents.take().unwrap_or_default(),
                cost_source: e
                    .cost_source
                    .take()
                    .and_then(|v| CostSource::from_str(&v).ok())
                    .unwrap_or_default(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
