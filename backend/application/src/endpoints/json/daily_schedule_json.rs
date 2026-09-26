use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-SC-02-S03` (`HRMS-605`): the daily schedule entry's HTTP shape.
/// Driver and vehicle are named by uuid (`HRMS-204`/`AD-010`), resolved at
/// the endpoint -- the same shape `TransportDemandAllocationJson` uses.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DailyScheduleJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub driver_uuid: String,
    pub vehicle_uuid: String,
    pub date: chrono::NaiveDate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_time: Option<chrono::NaiveTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<chrono::NaiveTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}
