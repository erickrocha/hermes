use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-GA-03-S01` (`HRMS-960`): a physical arrival or departure published by
/// the presence integration. `source` is `Tracker` (default); a
/// `ScheduleEstimate` is refused (`D-24(e)`).
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VehiclePresenceEventJson {
    pub vehicle_uuid: String,
    /// `Arrival` or `Departure`.
    pub event: String,
    /// Defaults to now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurred_at: Option<NaiveDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// The vehicle's physical stamps (`TRM-788`).
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VehiclePresenceJson {
    /// False when the event repeated the vehicle's current state and so changed nothing.
    pub recorded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_arrival_at: Option<NaiveDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_departure_at: Option<NaiveDateTime>,
    /// The latest event is a departure.
    pub away: bool,
}
