use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-SC-02-S04` (`HRMS-606`): the day exception's HTTP shape. The
/// replacement driver/vehicle are named by uuid (`HRMS-204`/`AD-010`),
/// resolved at the endpoint, and both optional -- a cancellation names
/// neither.
#[derive(Serialize, Deserialize, Debug, Clone, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleExceptionJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub date: chrono::NaiveDate,
    /// One of `Cancellation`, `Deallocation`, `Substitution`.
    pub exception_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_driver_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_vehicle_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// `EPIC-SC-03`'s trip id, once that domain exists (see the entity's own
    /// doc comment) -- carried through, not resolved from a uuid, since
    /// there is no trip endpoint yet to resolve it against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_trip_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}
