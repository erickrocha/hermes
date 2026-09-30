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
    /// `TRM-316`: the active extension's kilometre limit, if any. Read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension_limit_km: Option<f64>,
    /// `TRM-309`/`TRM-330`: the work order that last serviced the plan. On
    /// `PUT` it names the originating order of a new cycle (only read when
    /// the base changes); ignored on create.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_work_order_uuid: Option<String>,
    /// One of `Ok`, `Attention`, `Overdue`. Ignored on write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// `EPIC-MT-07-S02` (`HRMS-707`): the work order serving a due preventive
/// plan, and whether this call opened it.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PreventiveWorkOrderJson {
    pub work_order_uuid: Option<String>,
    pub number: Option<String>,
    pub created: bool,
}

/// `EPIC-MT-07-S05` (`HRMS-710`): extend a plan after a technical inspection
/// (`TRM-312…320`). `inspectionKm` defaults to the vehicle's current odometer
/// (`TRM-332`).
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PreventiveExtensionRequestJson {
    pub work_order_item_uuid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspection_km: Option<f64>,
    pub granted_km: f64,
    pub description: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PreventiveExtensionJson {
    pub uuid: Option<String>,
    pub inspection_km: f64,
    pub granted_km: f64,
    pub resulting_limit_km: f64,
    pub description: String,
}
