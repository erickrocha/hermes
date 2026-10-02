use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-GA-02-S01` (`HRMS-958`): open a triage for a vehicle.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GarageAttendanceOpenJson {
    pub vehicle_uuid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_priority: Option<i32>,
}

/// One service record of a triage. The stamps are server-written, one per state
/// (`TRM-438`).
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GarageServiceJson {
    pub uuid: Option<String>,
    pub service_model_uuid: Option<String>,
    pub name: String,
    pub service_group: String,
    pub required_for_departure: bool,
    /// The stored state, as last marked: one of `Pending`, `Performed`, `NotNeeded`, `NotDone`.
    pub state: String,
    /// `TRM-447`: what the state is **now** -- computed on every read from the vehicle's
    /// physical stamps, its marked trips and the tank, never stored. A performed service
    /// whose validity has lapsed reads `Pending` here.
    pub effective_state: String,
    /// Why: `Pending`, `ForcedPending`, `NoStamp`, `ConsumedByDeparture`, `ReturnedToBase`,
    /// `TripReturn`, `TripMarked`, `Expired`, `TankLow`, `TankGoverned` or `Valid`.
    pub effective_reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performed_at: Option<NaiveDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marked_at: Option<NaiveDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forced_pending_at: Option<NaiveDateTime>,
}

/// A triage -- the record that a vehicle is at base for services (`TRM-410`).
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GarageAttendanceJson {
    pub uuid: Option<String>,
    pub vehicle_uuid: String,
    pub attendance_date: NaiveDate,
    pub checked_in_at: NaiveDateTime,
    /// One of `Open`, `Finished`, `ReleasedWithPendency`, `LeftForOperation`.
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_priority: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released_at: Option<NaiveDateTime>,
    /// `Manual` or `ArrivalAtBase`.
    pub origin: String,
    /// `TRM-469…471`: the required services still effectively pending -- what blocks a departure.
    pub required_pending: Vec<String>,
    pub services: Vec<GarageServiceJson>,
}

/// `EPIC-GA-02-S02` (`HRMS-959`): the state a service is marked with.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GarageServiceMarkJson {
    /// One of `Pending`, `Performed`, `NotNeeded`, `NotDone`.
    pub state: String,
}

/// One vehicle of the yard's queue (`TRM-480`).
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GarageQueueEntryJson {
    pub attendance_uuid: Option<String>,
    pub vehicle_uuid: String,
    pub prefix: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_priority: Option<i32>,
    /// The vehicle's next trip departure inside the horizon (today, tomorrow; on Friday the weekend).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_departure: Option<NaiveDateTime>,
    pub required_pending: Vec<String>,
    /// `TRM-474`: visual only; it never reorders the queue (`TRM-476`).
    pub fill_tank_alert: bool,
    /// `TRM-485`: the manager has called this vehicle to base.
    pub called_by_manager: bool,
}

/// `EPIC-GA-07-S01` (`HRMS-964`): a queue entry as the wall monitor shows it.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GarageMonitorEntryJson {
    #[serde(flatten)]
    pub entry: GarageQueueEntryJson,
    /// `TRM-499`: `Alert`, `Preparing` or `Ready`; absent when no departure is scheduled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness: Option<String>,
    /// `TRM-498`: the urgent animation -- an extra trip inside its window with a required service pending.
    pub urgent: bool,
}

/// `EPIC-GA-07-S02` (`HRMS-965`): a vehicle card on the wall monitor.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GarageMonitorCardJson {
    pub vehicle_uuid: String,
    pub prefix: String,
    /// The estimated tank level; absent when no reading can be trusted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tank_percent: Option<f64>,
    pub needs_refuel: bool,
}

/// The cards shown and how many vehicles were left out (`TRM-495`).
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GarageMonitorCardsJson {
    pub cards: Vec<GarageMonitorCardJson>,
    pub not_shown: usize,
}

/// `EPIC-GA-07-S03` (`HRMS-966`): an away vehicle on the call-to-base list.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GarageCallListEntryJson {
    pub vehicle_uuid: String,
    pub prefix: String,
    /// The next departure still ahead (a started one is ignored, `TRM-487`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_departure: Option<NaiveDateTime>,
    /// `TRM-485`: the manager called it.
    pub called_by_manager: bool,
    /// `TRM-487`: a further departure is ahead.
    pub has_departure_ahead: bool,
    /// `TRM-1504`: the tank alone calls it.
    pub tank_calls: bool,
}

/// `EPIC-GA-07-S04` (`HRMS-967`, `TRM-496`): the wall monitor's services matrix.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GarageMatrixJson {
    pub columns: Vec<GarageMatrixColumnJson>,
    pub rows: Vec<GarageMatrixRowJson>,
    pub rows_not_shown: usize,
    pub columns_not_shown: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GarageMatrixColumnJson {
    pub service_model_uuid: String,
    pub name: String,
    /// External services are listed before internal ones.
    pub internal: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GarageMatrixRowJson {
    pub vehicle_uuid: String,
    pub prefix: String,
    /// One per column; `null` where the triage has no record of that service.
    pub cells: Vec<Option<GarageMatrixCellJson>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GarageMatrixCellJson {
    pub effective_state: String,
    /// The stamp of the stored state: performance, marking or forcing instant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stamp: Option<NaiveDateTime>,
    /// For a pending cell: the last time this service was performed on the vehicle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_performed_at: Option<NaiveDateTime>,
}

/// `EPIC-GA-05-S02` (`HRMS-963`, `TRM-483`): a vehicle to call to, or cancel a call for.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GarageCallRequestJson {
    pub vehicle_uuid: String,
}

/// The manual call in force for a vehicle.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GarageCallJson {
    pub active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub called_at: Option<NaiveDateTime>,
}
