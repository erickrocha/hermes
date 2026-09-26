use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::{WorkOrderOrigin, WorkOrderStatus};
use chrono::{NaiveDate, NaiveDateTime, Utc};
use entity::work_order_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};
use std::str::FromStr;

/// `EPIC-MT-01-S01` (`HRMS-700`): a maintenance work order.
/// `checklist_run_id` is set only for a `WorkOrderOrigin::Checklist` order
/// (`EPIC-CK-03-S02`, `TRM-115`: "link that work order back to the
/// checklist").
#[derive(Debug, Clone, PartialEq)]
pub struct WorkOrder {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub opened_at: Option<NaiveDateTime>,
    pub odometer_km: f64,
    pub origin: WorkOrderOrigin,
    pub checklist_run_id: Option<i64>,
    pub maintenance_plan_id: Option<i64>,
    pub service_type: Option<String>,
    pub description: String,
    pub responsible: Option<String>,
    pub status: WorkOrderStatus,
    pub observation: Option<String>,
    pub external_service: bool,
    pub supplier: Option<String>,
    pub invoice_number: Option<String>,
    pub invoice_value_cents: Option<i64>,
    pub invoice_date: Option<NaiveDate>,
    pub concluded_at: Option<NaiveDate>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct WorkOrderEntityMapper {}

impl EntityMapper<WorkOrder, Model, ActiveModel> for WorkOrderEntityMapper {
    fn build_active_model(d: WorkOrder) -> ActiveModel {
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
            // A caller states when the order was opened; absent means now.
            opened_at: Set(d.opened_at.map(|dt| dt.and_utc()).unwrap_or_else(Utc::now)),
            odometer_km: Set(d.odometer_km),
            origin: Set(d.origin.to_string()),
            checklist_run_id: Set(d.checklist_run_id),
            maintenance_plan_id: Set(d.maintenance_plan_id),
            service_type: Set(d.service_type),
            description: Set(d.description),
            responsible: Set(d.responsible),
            status: Set(d.status.to_string()),
            observation: Set(d.observation),
            external_service: Set(d.external_service),
            supplier: Set(d.supplier),
            invoice_number: Set(d.invoice_number),
            invoice_value_cents: Set(d.invoice_value_cents),
            invoice_date: Set(d.invoice_date),
            concluded_at: Set(d.concluded_at),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> WorkOrder {
        WorkOrder {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            vehicle_id: e.vehicle_id,
            opened_at: Some(e.opened_at.naive_utc()),
            odometer_km: e.odometer_km,
            origin: WorkOrderOrigin::from_str(&e.origin).unwrap_or(WorkOrderOrigin::Manual),
            checklist_run_id: e.checklist_run_id,
            maintenance_plan_id: e.maintenance_plan_id,
            service_type: e.service_type,
            description: e.description,
            responsible: e.responsible,
            status: WorkOrderStatus::from_str(&e.status).unwrap_or(WorkOrderStatus::Open),
            observation: e.observation,
            external_service: e.external_service,
            supplier: e.supplier,
            invoice_number: e.invoice_number,
            invoice_value_cents: e.invoice_value_cents,
            invoice_date: e.invoice_date,
            concluded_at: e.concluded_at,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> WorkOrder {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => WorkOrder {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                vehicle_id: e.vehicle_id.take().unwrap_or_default(),
                opened_at: e.opened_at.take().map(|dt| dt.naive_utc()),
                odometer_km: e.odometer_km.take().unwrap_or_default(),
                origin: e
                    .origin
                    .take()
                    .and_then(|value| WorkOrderOrigin::from_str(&value).ok())
                    .unwrap_or(WorkOrderOrigin::Manual),
                checklist_run_id: e.checklist_run_id.take().flatten(),
                maintenance_plan_id: e.maintenance_plan_id.take().flatten(),
                service_type: e.service_type.take().flatten(),
                description: e.description.take().unwrap_or_default(),
                responsible: e.responsible.take().flatten(),
                status: e
                    .status
                    .take()
                    .and_then(|value| WorkOrderStatus::from_str(&value).ok())
                    .unwrap_or(WorkOrderStatus::Open),
                observation: e.observation.take().flatten(),
                external_service: e.external_service.take().unwrap_or_default(),
                supplier: e.supplier.take().flatten(),
                invoice_number: e.invoice_number.take().flatten(),
                invoice_value_cents: e.invoice_value_cents.take().flatten(),
                invoice_date: e.invoice_date.take().flatten(),
                concluded_at: e.concluded_at.take().flatten(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
