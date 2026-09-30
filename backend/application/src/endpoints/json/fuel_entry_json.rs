use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-FU-01-S01` (`HRMS-942`): a vehicle's fuelling, manual entry only.
/// `origin` is server-derived (`Manual`) and ignored on write.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FuelEntryJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<i64>,
    pub vehicle_uuid: String,
    pub recorded_at: NaiveDateTime,
    pub volume_liters: f64,
    pub value_cents: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub odometer_km: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub station: Option<String>,
    #[serde(default)]
    pub full_tank: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

/// `EPIC-FU-02-S01` (`HRMS-944`): what one `POST /fuel-entry/sync` call did.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FuelSyncOutcomeJson {
    pub fetched: usize,
    pub imported: usize,
    pub updated: usize,
    pub unmatched: usize,
}

/// `EPIC-FU-08-S01` (`HRMS-945`): the fuelling report -- rows plus the totals
/// `TRM-1553` names.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FuelReportJson {
    pub entries: Vec<FuelEntryJson>,
    pub total_liters: f64,
    pub total_value_cents: i64,
}
