use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-SC-02-S02` (`HRMS-604`): the allocation's HTTP shape. Driver and
/// vehicle are named by uuid (`HRMS-204`/`AD-010`), resolved at the
/// endpoint -- the same shape `AssignDriverRequest` uses for
/// `vehicle_assignment`.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransportDemandAllocationJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub driver_uuid: String,
    pub vehicle_uuid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days_of_week: Option<String>,
    pub start_date: chrono::NaiveDate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_date: Option<chrono::NaiveDate>,
    #[serde(default = "default_true")]
    pub active: bool,
}

fn default_true() -> bool {
    true
}
