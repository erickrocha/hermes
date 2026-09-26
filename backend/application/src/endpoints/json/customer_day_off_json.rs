use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-SC-01-S02` (`HRMS-601`): the customer day-off's HTTP shape.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CustomerDayOffJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub date: chrono::NaiveDate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
