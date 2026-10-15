use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-MT-03-S01` (`HRMS-703`): a planned maintenance window's HTTP
/// shape. `status` and `origin` are server-controlled on create; the client
/// never sends them. `workOrderUuids` is the set of open work orders this
/// window covers -- linked, not embedded, since a work order still reads
/// through its own endpoint.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MaintenancePlanJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub vehicle_uuid: String,
    pub date: chrono::NaiveDate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planned_start: Option<chrono::NaiveDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planned_end: Option<chrono::NaiveDateTime>,
    /// One of `Scheduled`, `Concluded`, `Cancelled`, `NotExecuted`. Always
    /// `Scheduled` on a freshly created plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default)]
    pub affects_schedule: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(default)]
    pub work_order_uuids: Vec<String>,
}
