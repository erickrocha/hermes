use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-MT-07-S09` (`HRMS-714`): one answer of a technical inspection.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TechnicalInspectionItemJson {
    pub description: String,
    pub conforming: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation: Option<String>,
}

/// `EPIC-MT-07-S09` (`HRMS-714`, `TRM-333`): a technical inspection. `inspectedAt`
/// defaults to today and `odometerKm` to the vehicle's current odometer.
/// `workOrderUuids` and `workOrderOpened` are server-derived, ignored on write.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TechnicalInspectionJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub vehicle_uuid: String,
    pub inspection_model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspected_at: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub odometer_km: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation: Option<String>,
    pub items: Vec<TechnicalInspectionItemJson>,
    #[serde(default)]
    pub work_order_uuids: Vec<String>,
    #[serde(default)]
    pub work_order_opened: bool,
}
