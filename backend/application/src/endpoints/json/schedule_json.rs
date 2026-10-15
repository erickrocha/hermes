use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-SC-04-S01` (`HRMS-610`): one item of a day's effective schedule.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleItemJson {
    /// `Line`, `ExtraLine`, `OneOffTrip` or `ExtraTrip`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub demand_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<NaiveDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<NaiveDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vehicle_uuid: Option<String>,
    /// Where the crew came from: `Exception`, `DailyEntry`, `Allocation`, `DemandDefault` or `None` (`TRM-007`).
    pub resolved_from: String,
    /// `TRM-012`: an extra trip took a resource this line shares with it.
    pub displaced_by_trip: bool,
    /// `TRM-010`: `MissingDriver` and/or `MissingVehicle`. Covered only when empty.
    pub alerts: Vec<String>,
}

/// `TRM-001`: the effective schedule of a date, computed on demand from the demands,
/// allocations, day entries, exceptions and extra trips -- never stored.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveScheduleJson {
    pub date: chrono::NaiveDate,
    pub items: Vec<ScheduleItemJson>,
    /// Demands with no stated kind, left out rather than guessed at.
    pub unclassified_demands: usize,
    /// Recurring demands whose days of the week cannot be read, left out rather than guessed at.
    pub malformed_demands: usize,
}
