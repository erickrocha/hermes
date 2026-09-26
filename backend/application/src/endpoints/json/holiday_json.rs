use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-SC-01-S03` (`HRMS-602`): the holiday registry's HTTP shape.
#[derive(Serialize, Deserialize, Debug, Clone, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HolidayJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<i64>,
    pub date: chrono::NaiveDate,
    pub name: String,
}
