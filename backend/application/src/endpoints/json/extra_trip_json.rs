use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-SC-03-S01` (`HRMS-607`, `D-24(f)`): the canonical Trip's HTTP
/// shape. Every relation is named by uuid (`HRMS-204`/`AD-010`), resolved
/// at the endpoint, and all optional except the order code and date --
/// `D-24(d)`'s identity pair.
#[derive(Serialize, Deserialize, Debug, Clone, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExtraTripJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<i64>,
    pub order_code: String,
    pub trip_date: chrono::NaiveDate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customer_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_time: Option<chrono::NaiveTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_date: Option<chrono::NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_time: Option<chrono::NaiveTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_city: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stops: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_vehicle_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub second_driver_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vehicle_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freight_value_cents: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment: Option<String>,
    /// One of `Scheduled`, `Conflict`, `PendingSchedule`, `Cancelled`.
    #[serde(default)]
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import_batch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_at: Option<chrono::NaiveDateTime>,
    /// `EPIC-SC-03-S02` (CSV import) is not built yet; carried for forward
    /// compatibility with `substituida_por_id`'s replacement chain, resolved
    /// like every other relation once a trip endpoint exists to name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaced_by_uuid: Option<String>,
}
