use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-SC-02-S01` (`HRMS-603`): the transport-demand registry's HTTP
/// shape.
#[derive(Serialize, Deserialize, Debug, Clone, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransportDemandJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<i64>,
    pub demand_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customer_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shift_start: Option<chrono::NaiveTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shift_end: Option<chrono::NaiveTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days_of_week: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specific_date: Option<chrono::NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_vehicle_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_vehicle_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specific_driver_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specific_vehicle_uuid: Option<String>,
    #[serde(default = "default_true")]
    pub active: bool,
}

fn default_true() -> bool {
    true
}
