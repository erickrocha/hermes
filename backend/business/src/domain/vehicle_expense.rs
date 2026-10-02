use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::ExpenseOrigin;
use chrono::{NaiveDate, NaiveDateTime};
use entity::vehicle_expense_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};
use std::str::FromStr;

/// `EPIC-SP-04-S01` (`HRMS-803`): a direct vehicle expense -- toll, plan fee
/// or parking -- `entity-inventory.md` §4 "Despesas por veículo" (`TRM-660`,
/// `TRM-661`).
#[derive(Debug, Clone, PartialEq)]
pub struct VehicleExpense {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub category: String,
    pub competence_period: String,
    pub issue_date: NaiveDate,
    pub supplier: Option<String>,
    pub invoice_number: Option<String>,
    pub description: Option<String>,
    pub value_cents: i64,
    pub origin: ExpenseOrigin,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct VehicleExpenseEntityMapper {}

impl EntityMapper<VehicleExpense, Model, ActiveModel> for VehicleExpenseEntityMapper {
    fn build_active_model(d: VehicleExpense) -> ActiveModel {
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
            category: Set(d.category),
            competence_period: Set(d.competence_period),
            issue_date: Set(d.issue_date),
            supplier: Set(d.supplier),
            invoice_number: Set(d.invoice_number),
            description: Set(d.description),
            value_cents: Set(d.value_cents),
            origin: Set(d.origin.to_string()),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> VehicleExpense {
        VehicleExpense {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            vehicle_id: e.vehicle_id,
            category: e.category,
            competence_period: e.competence_period,
            issue_date: e.issue_date,
            supplier: e.supplier,
            invoice_number: e.invoice_number,
            description: e.description,
            value_cents: e.value_cents,
            origin: ExpenseOrigin::from_str(&e.origin).unwrap_or_default(),
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> VehicleExpense {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => VehicleExpense {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                vehicle_id: e.vehicle_id.take().unwrap_or_default(),
                category: e.category.take().unwrap_or_default(),
                competence_period: e.competence_period.take().unwrap_or_default(),
                issue_date: e.issue_date.take().unwrap_or_default(),
                supplier: e.supplier.take().flatten(),
                invoice_number: e.invoice_number.take().flatten(),
                description: e.description.take().flatten(),
                value_cents: e.value_cents.take().unwrap_or_default(),
                origin: e
                    .origin
                    .take()
                    .and_then(|v| ExpenseOrigin::from_str(&v).ok())
                    .unwrap_or_default(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
