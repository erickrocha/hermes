use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::tenant_select;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::{MaintenancePlanStatus, PresenceEventKind, WorkOrderStatus};
use crate::domain::maintenance_plan::{MaintenancePlan, MaintenancePlanEntityMapper};
use crate::domain::non_execution::{
    NonExecutionOccurrence, NonExecutionOccurrenceEntityMapper, NonExecutionRecord,
    NonExecutionRecordEntityMapper, NonExecutionView,
};
use crate::domain::work_order::{WorkOrder, WorkOrderEntityMapper};
use crate::use_cases::non_execution_evaluation::PresenceEvent;
use chrono::NaiveDateTime;
use entity::maintenance_plan_entity;
use entity::non_execution_occurrence_entity;
use entity::non_execution_record_entity;
use entity::prelude::{
    MaintenancePlanEntity as MaintenancePlanQuery,
    NonExecutionOccurrenceEntity as OccurrenceQuery, NonExecutionRecordEntity as RecordQuery,
    VehiclePresenceEventEntity as PresenceQuery, WorkOrderEntity as WorkOrderQuery,
};
use entity::vehicle_presence_event_entity;
use entity::work_order_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};
use std::collections::HashMap;
use std::str::FromStr;

#[async_trait]
pub trait NonExecutionStore: Send + Sync {
    async fn plans_in_status(
        &self,
        statuses: &[MaintenancePlanStatus],
    ) -> Result<Vec<MaintenancePlan>, BusinessError>;
    async fn find_plan(&self, id: i64) -> Result<Option<MaintenancePlan>, BusinessError>;
    async fn save_plan(&self, plan: MaintenancePlan) -> Result<MaintenancePlan, BusinessError>;
    async fn work_orders_of_plan(&self, plan_id: i64) -> Result<Vec<WorkOrder>, BusinessError>;
    async fn find_work_order_by_uuid(&self, uuid: String) -> Result<Option<WorkOrder>, BusinessError>;
    /// Oldest first.
    async fn presence_events(
        &self,
        vehicle_id: i64,
        from: NaiveDateTime,
        to: NaiveDateTime,
    ) -> Result<Vec<PresenceEvent>, BusinessError>;
    /// The vehicle's last event strictly before `before`, however old.
    async fn last_presence_before(
        &self,
        vehicle_id: i64,
        before: NaiveDateTime,
    ) -> Result<Option<PresenceEvent>, BusinessError>;
    async fn find_record_by_work_order(
        &self,
        work_order_id: i64,
    ) -> Result<Option<NonExecutionRecord>, BusinessError>;
    async fn save_record(&self, record: NonExecutionRecord) -> Result<NonExecutionRecord, BusinessError>;
    async fn occurrence_key_exists(&self, business_key: &str) -> Result<bool, BusinessError>;
    async fn save_occurrence(
        &self,
        occurrence: NonExecutionOccurrence,
    ) -> Result<NonExecutionOccurrence, BusinessError>;
    async fn find_occurrence_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<NonExecutionOccurrence>, BusinessError>;
    /// Every occurrence of the work order's record, oldest first, with the record and the order.
    async fn history_of_work_order(
        &self,
        work_order: WorkOrder,
    ) -> Result<Option<NonExecutionView>, BusinessError>;
    /// `SR-MT-04-08`: unresolved orders with at least one unjustified occurrence.
    async fn pending(&self) -> Result<Vec<NonExecutionView>, BusinessError>;
}

/// `EPIC-MT-04`: every read goes through `tenant_select`, same as every other gateway.
pub struct NonExecutionGateway {
    db: DbConn,
}

impl NonExecutionGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    async fn occurrences_of(&self, record_ids: Vec<i64>) -> Result<Vec<non_execution_occurrence_entity::Model>, DbErr> {
        tenant_select(OccurrenceQuery::find(), non_execution_occurrence_entity::Column::TenantId)
            .filter(non_execution_occurrence_entity::Column::RecordId.is_in(record_ids))
            .order_by_asc(non_execution_occurrence_entity::Column::WindowStart)
            .order_by_asc(non_execution_occurrence_entity::Column::Id)
            .all(&self.db)
            .await
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Database error: {error}");
    log::error!("[NonExecutionGateway] {message}");
    BusinessError::new(message)
}

fn presence_event(model: vehicle_presence_event_entity::Model) -> Option<PresenceEvent> {
    Some(PresenceEvent {
        kind: PresenceEventKind::from_str(&model.kind).ok()?,
        at: model.occurred_at.naive_utc(),
    })
}

#[async_trait]
impl NonExecutionStore for NonExecutionGateway {
    async fn plans_in_status(
        &self,
        statuses: &[MaintenancePlanStatus],
    ) -> Result<Vec<MaintenancePlan>, BusinessError> {
        let names: Vec<String> = statuses.iter().map(ToString::to_string).collect();
        tenant_select(MaintenancePlanQuery::find(), maintenance_plan_entity::Column::TenantId)
            .filter(maintenance_plan_entity::Column::Status.is_in(names))
            .order_by_asc(maintenance_plan_entity::Column::Id)
            .all(&self.db)
            .await
            .map(MaintenancePlanEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn find_plan(&self, id: i64) -> Result<Option<MaintenancePlan>, BusinessError> {
        tenant_select(MaintenancePlanQuery::find(), maintenance_plan_entity::Column::TenantId)
            .filter(maintenance_plan_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
            .map(|m| m.map(MaintenancePlanEntityMapper::from_model))
            .map_err(database_error)
    }

    async fn save_plan(&self, plan: MaintenancePlan) -> Result<MaintenancePlan, BusinessError> {
        MaintenancePlanEntityMapper::build_active_model(plan)
            .save(&self.db)
            .await
            .map(MaintenancePlanEntityMapper::from_active_model)
            .map_err(database_error)
    }

    async fn work_orders_of_plan(&self, plan_id: i64) -> Result<Vec<WorkOrder>, BusinessError> {
        tenant_select(WorkOrderQuery::find(), work_order_entity::Column::TenantId)
            .filter(work_order_entity::Column::MaintenancePlanId.eq(plan_id))
            .order_by_asc(work_order_entity::Column::Id)
            .all(&self.db)
            .await
            .map(WorkOrderEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn find_work_order_by_uuid(&self, uuid: String) -> Result<Option<WorkOrder>, BusinessError> {
        tenant_select(WorkOrderQuery::find(), work_order_entity::Column::TenantId)
            .filter(work_order_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
            .map(|m| m.map(WorkOrderEntityMapper::from_model))
            .map_err(database_error)
    }

    async fn presence_events(
        &self,
        vehicle_id: i64,
        from: NaiveDateTime,
        to: NaiveDateTime,
    ) -> Result<Vec<PresenceEvent>, BusinessError> {
        tenant_select(PresenceQuery::find(), vehicle_presence_event_entity::Column::TenantId)
            .filter(vehicle_presence_event_entity::Column::VehicleId.eq(vehicle_id))
            .filter(vehicle_presence_event_entity::Column::OccurredAt.gte(from.and_utc()))
            .filter(vehicle_presence_event_entity::Column::OccurredAt.lte(to.and_utc()))
            .order_by_asc(vehicle_presence_event_entity::Column::OccurredAt)
            .order_by_asc(vehicle_presence_event_entity::Column::Id)
            .all(&self.db)
            .await
            .map(|models| models.into_iter().filter_map(presence_event).collect())
            .map_err(database_error)
    }

    async fn last_presence_before(
        &self,
        vehicle_id: i64,
        before: NaiveDateTime,
    ) -> Result<Option<PresenceEvent>, BusinessError> {
        tenant_select(PresenceQuery::find(), vehicle_presence_event_entity::Column::TenantId)
            .filter(vehicle_presence_event_entity::Column::VehicleId.eq(vehicle_id))
            .filter(vehicle_presence_event_entity::Column::OccurredAt.lt(before.and_utc()))
            .order_by_desc(vehicle_presence_event_entity::Column::OccurredAt)
            .order_by_desc(vehicle_presence_event_entity::Column::Id)
            .one(&self.db)
            .await
            .map(|m| m.and_then(presence_event))
            .map_err(database_error)
    }

    async fn find_record_by_work_order(
        &self,
        work_order_id: i64,
    ) -> Result<Option<NonExecutionRecord>, BusinessError> {
        tenant_select(RecordQuery::find(), non_execution_record_entity::Column::TenantId)
            .filter(non_execution_record_entity::Column::WorkOrderId.eq(work_order_id))
            .one(&self.db)
            .await
            .map(|m| m.map(NonExecutionRecordEntityMapper::from_model))
            .map_err(database_error)
    }

    async fn save_record(&self, record: NonExecutionRecord) -> Result<NonExecutionRecord, BusinessError> {
        NonExecutionRecordEntityMapper::build_active_model(record)
            .save(&self.db)
            .await
            .map(NonExecutionRecordEntityMapper::from_active_model)
            .map_err(database_error)
    }

    async fn occurrence_key_exists(&self, business_key: &str) -> Result<bool, BusinessError> {
        tenant_select(OccurrenceQuery::find(), non_execution_occurrence_entity::Column::TenantId)
            .filter(non_execution_occurrence_entity::Column::BusinessKey.eq(business_key))
            .count(&self.db)
            .await
            .map(|count| count > 0)
            .map_err(database_error)
    }

    async fn save_occurrence(
        &self,
        occurrence: NonExecutionOccurrence,
    ) -> Result<NonExecutionOccurrence, BusinessError> {
        NonExecutionOccurrenceEntityMapper::build_active_model(occurrence)
            .save(&self.db)
            .await
            .map(NonExecutionOccurrenceEntityMapper::from_active_model)
            .map_err(database_error)
    }

    async fn find_occurrence_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<NonExecutionOccurrence>, BusinessError> {
        tenant_select(OccurrenceQuery::find(), non_execution_occurrence_entity::Column::TenantId)
            .filter(non_execution_occurrence_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
            .map(|m| m.map(NonExecutionOccurrenceEntityMapper::from_model))
            .map_err(database_error)
    }

    async fn history_of_work_order(
        &self,
        work_order: WorkOrder,
    ) -> Result<Option<NonExecutionView>, BusinessError> {
        let Some(order_id) = work_order.id else {
            return Ok(None);
        };
        let Some(record) = self.find_record_by_work_order(order_id).await? else {
            return Ok(None);
        };
        let Some(record_id) = record.id else {
            return Ok(None);
        };
        let occurrences = NonExecutionOccurrenceEntityMapper::from_models(
            self.occurrences_of(vec![record_id]).await.map_err(database_error)?,
        );
        let total_occurrences = occurrences.len() as u64;
        Ok(Some(NonExecutionView { record, work_order, occurrences, total_occurrences }))
    }

    async fn pending(&self) -> Result<Vec<NonExecutionView>, BusinessError> {
        let open = tenant_select(OccurrenceQuery::find(), non_execution_occurrence_entity::Column::TenantId)
            .filter(non_execution_occurrence_entity::Column::JustifiedAt.is_null())
            .all(&self.db)
            .await
            .map_err(database_error)?;
        let mut record_ids: Vec<i64> = open.iter().map(|o| o.record_id).collect();
        record_ids.sort_unstable();
        record_ids.dedup();
        if record_ids.is_empty() {
            return Ok(Vec::new());
        }
        let records = tenant_select(RecordQuery::find(), non_execution_record_entity::Column::TenantId)
            .filter(non_execution_record_entity::Column::Id.is_in(record_ids.clone()))
            .all(&self.db)
            .await
            .map_err(database_error)?;
        let order_ids: Vec<i64> = records.iter().map(|r| r.work_order_id).collect();
        let unresolved = [WorkOrderStatus::Open, WorkOrderStatus::PartiallyResolved, WorkOrderStatus::AwaitingParts]
            .map(|s| s.to_string());
        let orders: HashMap<i64, work_order_entity::Model> =
            tenant_select(WorkOrderQuery::find(), work_order_entity::Column::TenantId)
                .filter(work_order_entity::Column::Id.is_in(order_ids))
                .filter(work_order_entity::Column::Status.is_in(unresolved))
                .all(&self.db)
                .await
                .map_err(database_error)?
                .into_iter()
                .map(|o| (o.id, o))
                .collect();
        let mut occurrences: HashMap<i64, Vec<NonExecutionOccurrence>> = HashMap::new();
        for model in self.occurrences_of(record_ids).await.map_err(database_error)? {
            occurrences
                .entry(model.record_id)
                .or_default()
                .push(NonExecutionOccurrenceEntityMapper::from_model(model));
        }
        let mut views = Vec::new();
        for record in records {
            let Some(order) = orders.get(&record.work_order_id) else {
                continue;
            };
            let all = occurrences.remove(&record.id).unwrap_or_default();
            let total_occurrences = all.len() as u64;
            views.push(NonExecutionView {
                record: NonExecutionRecordEntityMapper::from_model(record),
                work_order: WorkOrderEntityMapper::from_model(order.clone()),
                occurrences: all.into_iter().filter(|o| o.justified_at.is_none()).collect(),
                total_occurrences,
            });
        }
        views.sort_by_key(|v| (v.record.vehicle_id, v.record.work_order_id));
        Ok(views)
    }
}
