use crate::commons::functions::bytes_para_string;
use crate::commons::gateway::tenant_select;
use crate::domain::business_error::BusinessError;
use crate::domain::cost_report::{DatedLine, SupplierLine, VehicleCostInput, WorkOrderLine, work_order_date};
use crate::domain::enums::{GarageServiceState, TripStatus, WorkOrderStatus};
use chrono::NaiveDate;
use entity::prelude::{
    ExtraTripEntity, FuelEntryEntity, GarageServiceLogEntity, GarageServiceModelEntity, VehicleEntity, VehicleExpenseEntity,
    WorkOrderEntity, WorkOrderPostingEntity,
};
use entity::{
    extra_trip_entity, fuel_entry_entity, garage_service_log_entity, garage_service_model_entity, vehicle_entity,
    vehicle_expense_entity, work_order_entity, work_order_posting_entity,
};
use sea_orm::{ColumnTrait, DbConn, DbErr, EntityTrait, QueryFilter};
use std::collections::HashMap;

/// `C-030` `EPIC-SP-07`/`SP-08`: read-only queries behind the cost report, every one through `tenant_select`.
/// shortcut: rows are filtered by period in memory after a per-vehicle (or per-supplier) read -- fine for a
/// vehicle's own history; push the date filter into SQL if a tenant's history outgrows it.
pub struct CostReportGateway {
    db: DbConn,
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Cost report database error: {error}");
    log::error!("[CostReportGateway] {message}");
    BusinessError::new(message)
}

fn within(date: NaiveDate, from: NaiveDate, to: NaiveDate) -> bool {
    date >= from && date <= to
}

impl CostReportGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    async fn postings_by_order(&self, order_ids: Vec<i64>) -> Result<HashMap<i64, i64>, BusinessError> {
        if order_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let postings = tenant_select(WorkOrderPostingEntity::find(), work_order_posting_entity::Column::TenantId)
            .filter(work_order_posting_entity::Column::WorkOrderId.is_in(order_ids))
            .all(&self.db)
            .await
            .map_err(database_error)?;
        let mut totals: HashMap<i64, i64> = HashMap::new();
        for posting in postings {
            *totals.entry(posting.work_order_id).or_default() += posting.total_value_cents;
        }
        Ok(totals)
    }

    fn order_line(order: &work_order_entity::Model, postings: i64) -> WorkOrderLine {
        WorkOrderLine {
            uuid: bytes_para_string(order.uuid.clone()),
            date: work_order_date(order.concluded_at, order.opened_at.naive_utc()),
            service_type: order.service_type.clone(),
            description: order.description.clone(),
            postings_cents: postings,
            external_cents: if order.external_service { order.invoice_value_cents.unwrap_or(0) } else { 0 },
        }
    }

    /// Everything a vehicle cost and earned over the period (`HRMS-804…807`). A cancelled work order, a soft-deleted
    /// or unified fuelling and a cancelled trip are not counted.
    pub async fn vehicle_input(&self, vehicle_id: i64, from: NaiveDate, to: NaiveDate) -> Result<VehicleCostInput, BusinessError> {
        let orders = tenant_select(WorkOrderEntity::find(), work_order_entity::Column::TenantId)
            .filter(work_order_entity::Column::VehicleId.eq(vehicle_id))
            .filter(work_order_entity::Column::Status.ne(WorkOrderStatus::Cancelled.to_string()))
            .all(&self.db)
            .await
            .map_err(database_error)?;
        let orders: Vec<_> = orders
            .into_iter()
            .filter(|o| within(work_order_date(o.concluded_at, o.opened_at.naive_utc()), from, to))
            .collect();
        let postings = self.postings_by_order(orders.iter().map(|o| o.id).collect()).await?;
        let work_orders = orders.iter().map(|o| Self::order_line(o, postings.get(&o.id).copied().unwrap_or(0))).collect();

        let fuel = tenant_select(FuelEntryEntity::find(), fuel_entry_entity::Column::TenantId)
            .filter(fuel_entry_entity::Column::VehicleId.eq(vehicle_id))
            .filter(fuel_entry_entity::Column::DeletedAt.is_null())
            .filter(fuel_entry_entity::Column::UnifiedIntoId.is_null())
            .all(&self.db)
            .await
            .map_err(database_error)?
            .into_iter()
            .filter(|f| within(f.recorded_at.date_naive(), from, to))
            .map(|f| DatedLine {
                date: f.recorded_at.date_naive(),
                reference: bytes_para_string(f.uuid),
                description: f.station.unwrap_or_default(),
                value_cents: f.value_cents,
            })
            .collect();

        // A competence period is a month, "YYYY-MM": it is in the report when that month touches the period.
        let (first_month, last_month) = (from.format("%Y-%m").to_string(), to.format("%Y-%m").to_string());
        let expenses = tenant_select(VehicleExpenseEntity::find(), vehicle_expense_entity::Column::TenantId)
            .filter(vehicle_expense_entity::Column::VehicleId.eq(vehicle_id))
            .all(&self.db)
            .await
            .map_err(database_error)?
            .into_iter()
            .filter(|e| e.competence_period.as_str() >= first_month.as_str() && e.competence_period.as_str() <= last_month.as_str())
            .map(|e| DatedLine {
                date: e.issue_date,
                reference: bytes_para_string(e.uuid),
                description: [Some(e.category), e.supplier, e.description].into_iter().flatten().collect::<Vec<_>>().join(" - "),
                value_cents: e.value_cents,
            })
            .collect();

        let trips = tenant_select(ExtraTripEntity::find(), extra_trip_entity::Column::TenantId)
            .filter(extra_trip_entity::Column::VehicleId.eq(vehicle_id))
            .filter(extra_trip_entity::Column::Status.ne(TripStatus::Cancelled.to_string()))
            .all(&self.db)
            .await
            .map_err(database_error)?;
        let priced: Vec<i64> = trips
            .iter()
            .filter(|t| within(t.trip_date, from, to))
            .filter_map(|t| t.freight_value_cents)
            .collect();

        let logs = tenant_select(GarageServiceLogEntity::find(), garage_service_log_entity::Column::TenantId)
            .filter(garage_service_log_entity::Column::VehicleId.eq(vehicle_id))
            .filter(garage_service_log_entity::Column::NewState.eq(GarageServiceState::Performed.to_string()))
            .all(&self.db)
            .await
            .map_err(database_error)?;
        let names: HashMap<i64, String> = tenant_select(GarageServiceModelEntity::find(), garage_service_model_entity::Column::TenantId)
            .all(&self.db)
            .await
            .map_err(database_error)?
            .into_iter()
            .map(|m| (m.id, m.name))
            .collect();
        let mut performed: HashMap<String, i64> = HashMap::new();
        for log in logs.iter().filter(|l| within(l.acted_at.date_naive(), from, to)) {
            let name = names.get(&log.service_model_id).cloned().unwrap_or_else(|| log.name_key.clone());
            *performed.entry(name).or_default() += 1;
        }
        let mut services: Vec<(String, i64)> = performed.into_iter().collect();
        services.sort();

        Ok(VehicleCostInput {
            work_orders,
            fuel,
            expenses,
            freight_cents: priced.iter().sum(),
            freight_trips: priced.len() as i64,
            services,
        })
    }

    pub async fn vehicle_plate(&self, vehicle_id: i64) -> Result<String, BusinessError> {
        Ok(tenant_select(VehicleEntity::find(), vehicle_entity::Column::TenantId)
            .filter(vehicle_entity::Column::Id.eq(vehicle_id))
            .one(&self.db)
            .await
            .map_err(database_error)?
            .map(|v| v.plate)
            .unwrap_or_default())
    }

    /// `HRMS-808`: (postings, external-service value) of one work order; `None` if it is not visible.
    pub async fn work_order_cost(&self, uuid: String) -> Result<Option<(i64, i64)>, BusinessError> {
        let Some(order) = tenant_select(WorkOrderEntity::find(), work_order_entity::Column::TenantId)
            .filter(work_order_entity::Column::Uuid.eq(crate::commons::functions::string_to_bytes(&uuid)))
            .one(&self.db)
            .await
            .map_err(database_error)?
        else {
            return Ok(None);
        };
        let postings = self.postings_by_order(vec![order.id]).await?.get(&order.id).copied().unwrap_or(0);
        let line = Self::order_line(&order, postings);
        Ok(Some((line.postings_cents, line.external_cents)))
    }

    async fn external_orders(&self) -> Result<Vec<work_order_entity::Model>, BusinessError> {
        tenant_select(WorkOrderEntity::find(), work_order_entity::Column::TenantId)
            .filter(work_order_entity::Column::ExternalService.eq(true))
            .filter(work_order_entity::Column::Status.ne(WorkOrderStatus::Cancelled.to_string()))
            .all(&self.db)
            .await
            .map_err(database_error)
    }

    /// `HRMS-810`: the supplier names actually used on external-service work orders.
    pub async fn suppliers(&self) -> Result<Vec<String>, BusinessError> {
        let mut names: Vec<String> = self
            .external_orders()
            .await?
            .into_iter()
            .filter_map(|o| o.supplier)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        names.sort_by_key(|s| s.to_lowercase());
        names.dedup_by_key(|s| s.to_lowercase());
        Ok(names)
    }

    pub async fn supplier_lines(&self, supplier: &str, from: NaiveDate, to: NaiveDate) -> Result<Vec<SupplierLine>, BusinessError> {
        let wanted = supplier.trim().to_lowercase();
        let orders: Vec<_> = self
            .external_orders()
            .await?
            .into_iter()
            .filter(|o| o.supplier.as_deref().map(|s| s.trim().to_lowercase()) == Some(wanted.clone()))
            .filter(|o| within(work_order_date(o.concluded_at, o.opened_at.naive_utc()), from, to))
            .collect();
        let plates: HashMap<i64, String> = tenant_select(VehicleEntity::find(), vehicle_entity::Column::TenantId)
            .filter(vehicle_entity::Column::Id.is_in(orders.iter().map(|o| o.vehicle_id).collect::<Vec<_>>()))
            .all(&self.db)
            .await
            .map_err(database_error)?
            .into_iter()
            .map(|v| (v.id, v.plate))
            .collect();
        let mut lines: Vec<SupplierLine> = orders
            .iter()
            .map(|o| SupplierLine {
                date: work_order_date(o.concluded_at, o.opened_at.naive_utc()),
                invoice_number: o.invoice_number.clone(),
                service: o.description.clone(),
                work_order_type: o.service_type.clone(),
                plate: plates.get(&o.vehicle_id).cloned().unwrap_or_default(),
                invoice_cents: o.invoice_value_cents.unwrap_or(0),
            })
            .collect();
        lines.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.invoice_number.cmp(&b.invoice_number)));
        Ok(lines)
    }
}
