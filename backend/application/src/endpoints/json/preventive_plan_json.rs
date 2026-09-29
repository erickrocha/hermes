use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-MT-07-S01` (`HRMS-706`): a vehicle's preventive-maintenance plan.
/// `status` is always server-derived (`TRM-302`/`303`) from the vehicle's
/// current odometer and today's date -- never stored, ignored on write.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PreventivePlanJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<i64>,
    pub vehicle_uuid: String,
    pub plan_name: String,
    /// One of `Kilometers`, `Days`, `Both` (`TRM-301`).
    pub control_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval_km: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval_days: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_service_km: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_service_date: Option<NaiveDate>,
    /// One of `Ok`, `Attention`, `Overdue`. Ignored on write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}
