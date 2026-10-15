use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::NaiveDateTime;
use entity::non_execution_occurrence_entity as occurrence_entity;
use entity::non_execution_record_entity as record_entity;
use sea_orm::{NotSet, Set, TryIntoModel};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// `EPIC-MT-04` (`C-026`): how a planned maintenance went unexecuted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonExecutionKind {
    /// The vehicle came to the base and left with the work order unresolved (`TRM-261`).
    EnteredAndLeft,
    /// The window passed with evidence the vehicle never came (`TRM-262`).
    DidNotAppear,
    /// Recorded by hand, backed by presence evidence (`TRM-272`).
    Manual,
}

impl Display for NonExecutionKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            NonExecutionKind::EnteredAndLeft => "EnteredAndLeft",
            NonExecutionKind::DidNotAppear => "DidNotAppear",
            NonExecutionKind::Manual => "Manual",
        })
    }
}

impl FromStr for NonExecutionKind {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "EnteredAndLeft" => Ok(Self::EnteredAndLeft),
            "DidNotAppear" => Ok(Self::DidNotAppear),
            "Manual" => Ok(Self::Manual),
            _ => Err(format!("Invalid non-execution kind: {value}")),
        }
    }
}

/// `SR-MT-04-07`: the single history record of a work order.
#[derive(Debug, Clone, PartialEq)]
pub struct NonExecutionRecord {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub work_order_id: i64,
    pub vehicle_id: i64,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

/// One non-execution. `business_key` is unique per tenant (`SR-MT-04-06`).
#[derive(Debug, Clone, PartialEq)]
pub struct NonExecutionOccurrence {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub record_id: i64,
    pub maintenance_plan_id: i64,
    pub kind: NonExecutionKind,
    pub business_key: String,
    pub window_start: NaiveDateTime,
    pub window_end: NaiveDateTime,
    pub arrived_at: Option<NaiveDateTime>,
    pub departed_at: Option<NaiveDateTime>,
    pub dwell_minutes: Option<i32>,
    pub minutes_in_window: Option<i32>,
    pub justification: Option<String>,
    pub justified_by: Option<String>,
    pub justified_at: Option<NaiveDateTime>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct NonExecutionRecordEntityMapper {}

impl EntityMapper<NonExecutionRecord, record_entity::Model, record_entity::ActiveModel>
    for NonExecutionRecordEntityMapper
{
    fn build_active_model(d: NonExecutionRecord) -> record_entity::ActiveModel {
        record_entity::ActiveModel {
            id: d.id.map(Set).unwrap_or(NotSet),
            uuid: d.uuid.map(|u| Set(string_to_bytes(&u))).unwrap_or(NotSet),
            tenant_id: Set(d.tenant_id),
            work_order_id: Set(d.work_order_id),
            vehicle_id: Set(d.vehicle_id),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: record_entity::Model) -> NonExecutionRecord {
        NonExecutionRecord {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            work_order_id: e.work_order_id,
            vehicle_id: e.vehicle_id,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(e: record_entity::ActiveModel) -> NonExecutionRecord {
        // A saved active model always carries every column.
        let model: Result<record_entity::Model, _> = e.try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => NonExecutionRecord {
                id: None,
                uuid: None,
                tenant_id: None,
                work_order_id: 0,
                vehicle_id: 0,
                created_at: None,
                created_by: None,
                updated_at: None,
                updated_by: None,
            },
        }
    }
}

pub struct NonExecutionOccurrenceEntityMapper {}

impl
    EntityMapper<
        NonExecutionOccurrence,
        occurrence_entity::Model,
        occurrence_entity::ActiveModel,
    > for NonExecutionOccurrenceEntityMapper
{
    fn build_active_model(d: NonExecutionOccurrence) -> occurrence_entity::ActiveModel {
        occurrence_entity::ActiveModel {
            id: d.id.map(Set).unwrap_or(NotSet),
            uuid: d.uuid.map(|u| Set(string_to_bytes(&u))).unwrap_or(NotSet),
            tenant_id: Set(d.tenant_id),
            record_id: Set(d.record_id),
            maintenance_plan_id: Set(d.maintenance_plan_id),
            kind: Set(d.kind.to_string()),
            business_key: Set(d.business_key),
            window_start: Set(d.window_start.and_utc()),
            window_end: Set(d.window_end.and_utc()),
            arrived_at: Set(d.arrived_at.map(|t| t.and_utc())),
            departed_at: Set(d.departed_at.map(|t| t.and_utc())),
            dwell_minutes: Set(d.dwell_minutes),
            minutes_in_window: Set(d.minutes_in_window),
            justification: Set(d.justification),
            justified_by: Set(d.justified_by),
            justified_at: Set(d.justified_at.map(|t| t.and_utc())),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: occurrence_entity::Model) -> NonExecutionOccurrence {
        NonExecutionOccurrence {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            record_id: e.record_id,
            maintenance_plan_id: e.maintenance_plan_id,
            kind: NonExecutionKind::from_str(&e.kind).unwrap_or(NonExecutionKind::Manual),
            business_key: e.business_key,
            window_start: e.window_start.naive_utc(),
            window_end: e.window_end.naive_utc(),
            arrived_at: e.arrived_at.map(|t| t.naive_utc()),
            departed_at: e.departed_at.map(|t| t.naive_utc()),
            dwell_minutes: e.dwell_minutes,
            minutes_in_window: e.minutes_in_window,
            justification: e.justification,
            justified_by: e.justified_by,
            justified_at: e.justified_at.map(|t| t.naive_utc()),
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(e: occurrence_entity::ActiveModel) -> NonExecutionOccurrence {
        let model: Result<occurrence_entity::Model, _> = e.try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => NonExecutionOccurrence {
                id: None,
                uuid: None,
                tenant_id: None,
                record_id: 0,
                maintenance_plan_id: 0,
                kind: NonExecutionKind::Manual,
                business_key: String::new(),
                window_start: NaiveDateTime::default(),
                window_end: NaiveDateTime::default(),
                arrived_at: None,
                departed_at: None,
                dwell_minutes: None,
                minutes_in_window: None,
                justification: None,
                justified_by: None,
                justified_at: None,
                created_at: None,
                created_by: None,
                updated_at: None,
                updated_by: None,
            },
        }
    }
}

/// A work order with its non-execution history: the inbox row (only the unjustified occurrences
/// in `occurrences`) or the full history (every occurrence). `total_occurrences` is always the
/// whole history (`PR-MT-04-05`).
#[derive(Debug, Clone, PartialEq)]
pub struct NonExecutionView {
    pub record: NonExecutionRecord,
    pub work_order: crate::domain::work_order::WorkOrder,
    pub occurrences: Vec<NonExecutionOccurrence>,
    pub total_occurrences: u64,
}

/// What one evaluation pass did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EvaluationOutcome {
    pub plans_evaluated: u64,
    pub occurrences_recorded: u64,
    pub plans_concluded: u64,
}
