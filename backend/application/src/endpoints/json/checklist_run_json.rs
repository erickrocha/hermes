use crate::endpoints::json::checklist_answer_json::ChecklistAnswerJson;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-CK-03-S01`/`EPIC-CK-04-S01` (`HRMS-652`/`654`): a submitted driver
/// checklist's HTTP shape. `openingChecklistUuid` is set only on a `Return`,
/// naming the `Departure` it closes. `answers` is required on submission
/// (`TRM-101`: no items is refused) and always populated on a read.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistRunJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub checklist_template_uuid: String,
    pub driver_uuid: String,
    pub vehicle_uuid: String,
    /// One of `Departure`, `Return`, `Standalone`.
    pub checklist_type: String,
    pub odometer_km: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opening_checklist_uuid: Option<String>,
    #[serde(default)]
    pub answers: Vec<ChecklistAnswerJson>,
}
