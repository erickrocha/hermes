use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-MT-04` (`C-026`): one time the planned maintenance was not carried out. The
/// justification fields appear only once the occurrence is justified.
#[derive(Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NonExecutionOccurrenceJson {
    pub uuid: String,
    /// `EnteredAndLeft`, `DidNotAppear` or `Manual`
    pub kind: String,
    pub window_start: NaiveDateTime,
    pub window_end: NaiveDateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrived_at: Option<NaiveDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub departed_at: Option<NaiveDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dwell_minutes: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minutes_in_window: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub justification: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub justified_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub justified_at: Option<NaiveDateTime>,
}

#[derive(Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NonExecutionVehicleJson {
    pub uuid: String,
    pub plate: String,
    pub model: String,
}

/// One row of the inbox: a work order, how many times it went unresolved in all (`totalOccurrences`)
/// and the occurrences still to justify.
#[derive(Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NonExecutionWorkOrderJson {
    pub work_order_uuid: String,
    pub description: String,
    pub status: String,
    pub total_occurrences: u64,
    pub occurrences: Vec<NonExecutionOccurrenceJson>,
}

/// The inbox, grouped by vehicle.
#[derive(Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NonExecutionVehicleGroupJson {
    pub vehicle: NonExecutionVehicleJson,
    pub work_orders: Vec<NonExecutionWorkOrderJson>,
}

/// A work order's whole history, justified occurrences included.
#[derive(Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NonExecutionHistoryJson {
    pub vehicle: NonExecutionVehicleJson,
    pub work_order: NonExecutionWorkOrderJson,
}

#[derive(Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JustifyJson {
    /// 1 to 1000 characters
    pub justification: String,
}

#[derive(Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManualNonExecutionJson {
    pub work_order_uuid: String,
    /// 1 to 1000 characters
    pub justification: String,
}

#[derive(Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationJson {
    pub plans_evaluated: u64,
    pub occurrences_recorded: u64,
    pub plans_concluded: u64,
}
