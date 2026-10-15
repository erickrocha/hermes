use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::{CostSource, StockMovementType};
use chrono::{NaiveDate, NaiveDateTime};
use entity::stock_movement_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};
use std::str::FromStr;

/// `EPIC-SP-02-S01` (`HRMS-801`): one entry in a part's stock ledger --
/// `entity-inventory.md` §4 "Movimentações de estoque" (`TRM-602…612`).
#[derive(Debug, Clone, PartialEq)]
pub struct StockMovement {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub part_id: i64,
    pub movement_type: StockMovementType,
    pub quantity: f64,
    pub unit_value_cents: Option<i64>,
    pub total_value_cents: Option<i64>,
    pub cost_source: CostSource,
    pub supplier: Option<String>,
    pub invoice_number: Option<String>,
    pub entry_date: Option<NaiveDate>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct StockMovementEntityMapper {}

impl EntityMapper<StockMovement, Model, ActiveModel> for StockMovementEntityMapper {
    fn build_active_model(d: StockMovement) -> ActiveModel {
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
            movement_type: Set(d.movement_type.to_string()),
            quantity: Set(d.quantity),
            unit_value_cents: Set(d.unit_value_cents),
            total_value_cents: Set(d.total_value_cents),
            cost_source: Set(d.cost_source.to_string()),
            supplier: Set(d.supplier),
            invoice_number: Set(d.invoice_number),
            entry_date: Set(d.entry_date),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> StockMovement {
        StockMovement {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            part_id: e.part_id,
            movement_type: StockMovementType::from_str(&e.movement_type).unwrap_or_default(),
            quantity: e.quantity,
            unit_value_cents: e.unit_value_cents,
            total_value_cents: e.total_value_cents,
            cost_source: CostSource::from_str(&e.cost_source).unwrap_or_default(),
            supplier: e.supplier,
            invoice_number: e.invoice_number,
            entry_date: e.entry_date,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> StockMovement {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => StockMovement {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                part_id: e.part_id.take().unwrap_or_default(),
                movement_type: e
                    .movement_type
                    .take()
                    .and_then(|v| StockMovementType::from_str(&v).ok())
                    .unwrap_or_default(),
                quantity: e.quantity.take().unwrap_or_default(),
                unit_value_cents: e.unit_value_cents.take().flatten(),
                total_value_cents: e.total_value_cents.take().flatten(),
                cost_source: e
                    .cost_source
                    .take()
                    .and_then(|v| CostSource::from_str(&v).ok())
                    .unwrap_or_default(),
                supplier: e.supplier.take().flatten(),
                invoice_number: e.invoice_number.take().flatten(),
                entry_date: e.entry_date.take().flatten(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
