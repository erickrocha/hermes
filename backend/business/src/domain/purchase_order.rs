use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::PurchaseOrderStatus;
use chrono::{NaiveDate, NaiveDateTime};
use entity::purchase_order_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};
use std::str::FromStr;

/// `EPIC-SP-03-S01` (`HRMS-802`): a request to buy a part --
/// `entity-inventory.md` §4 "Pedidos de compra" (`TRM-640…643`, `651…657`).
#[derive(Debug, Clone, PartialEq)]
pub struct PurchaseOrder {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub part_id: i64,
    pub quantity: f64,
    pub suggested_supplier: Option<String>,
    pub observation: Option<String>,
    pub status: PurchaseOrderStatus,
    pub work_order_id: Option<i64>,
    pub work_order_item_id: Option<i64>,
    pub vehicle_id: Option<i64>,
    pub ordered_at: Option<NaiveDate>,
    pub expected_delivery_date: Option<NaiveDate>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct PurchaseOrderEntityMapper {}

impl EntityMapper<PurchaseOrder, Model, ActiveModel> for PurchaseOrderEntityMapper {
    fn build_active_model(d: PurchaseOrder) -> ActiveModel {
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
            part_id: Set(d.part_id),
            quantity: Set(d.quantity),
            suggested_supplier: Set(d.suggested_supplier),
            observation: Set(d.observation),
            status: Set(d.status.to_string()),
            work_order_id: Set(d.work_order_id),
            work_order_item_id: Set(d.work_order_item_id),
            vehicle_id: Set(d.vehicle_id),
            ordered_at: Set(d.ordered_at),
            expected_delivery_date: Set(d.expected_delivery_date),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> PurchaseOrder {
        PurchaseOrder {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            part_id: e.part_id,
            quantity: e.quantity,
            suggested_supplier: e.suggested_supplier,
            observation: e.observation,
            status: PurchaseOrderStatus::from_str(&e.status).unwrap_or_default(),
            work_order_id: e.work_order_id,
            work_order_item_id: e.work_order_item_id,
            vehicle_id: e.vehicle_id,
            ordered_at: e.ordered_at,
            expected_delivery_date: e.expected_delivery_date,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> PurchaseOrder {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => PurchaseOrder {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                part_id: e.part_id.take().unwrap_or_default(),
                quantity: e.quantity.take().unwrap_or_default(),
                suggested_supplier: e.suggested_supplier.take().flatten(),
                observation: e.observation.take().flatten(),
                status: e
                    .status
                    .take()
                    .and_then(|v| PurchaseOrderStatus::from_str(&v).ok())
                    .unwrap_or_default(),
                work_order_id: e.work_order_id.take().flatten(),
                work_order_item_id: e.work_order_item_id.take().flatten(),
                vehicle_id: e.vehicle_id.take().flatten(),
                ordered_at: e.ordered_at.take().flatten(),
                expected_delivery_date: e.expected_delivery_date.take().flatten(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
