use serde::{Deserialize, Serialize};
use business::domain::garage_service_model::GarageServiceApplicability;
use utoipa::ToSchema;

/// `EPIC-GA-01-S01` (`HRMS-956`, `TRM-430`/`431`/`470`): a garage service of the
/// tenant's catalogue. `serviceGroup` is `External` or `Internal`, stated
/// explicitly. `requiredForDeparture` is an attribute of the entry, never
/// derived from its name. `active` defaults to true. `applicability` defaults
/// to all vehicles and may target free-text vehicle types or explicit UUIDs.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GarageServiceModelJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<i64>,
    pub name: String,
    #[serde(default)]
    pub display_order: i32,
    #[serde(default = "yes")]
    pub active: bool,
    pub service_group: String,
    #[serde(default)]
    pub required_for_departure: bool,
    /// `TRM-462`: the service is governed by the tank level, not by elapsed time.
    #[serde(default)]
    pub governed_by_tank: bool,
    #[serde(default)]
    pub applicability: GarageServiceApplicability,
}

fn yes() -> bool {
    true
}
