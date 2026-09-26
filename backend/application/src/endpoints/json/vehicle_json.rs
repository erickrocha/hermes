use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// EPIC-FO-01-S05 (HRMS-924): the vehicle register's HTTP shape.
#[derive(Serialize, Deserialize, Debug, Clone, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VehicleJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    /// Only an unbound platform administrator may name it; a tenant owner's
    /// vehicles are always written into their own tenant (HRMS-921).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<i64>,
    pub plate: String,
    pub model: String,
    /// One of `Active`, `Maintenance`, `Transit`, `Reserved`, `Inactive`
    /// (HRMS-922, D-23(b)). Anything else is refused, never defaulted.
    #[serde(default)]
    pub status: String,
    /// The tracking provider's device id (HRMS-926). Only the platform
    /// administrator may set or change it; unique across the platform.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracker_device_id: Option<i64>,
    /// `C-023`/`HRMS-941`: operacao-trm parity fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vehicle_type: Option<String>,
    /// Read-only in practice today: no writer exists yet (`C-025`). Not
    /// rejected if sent, simply carried through like any other field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub odometer_km: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wheel_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spare_tire_count: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spare_tire_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spare_tire_notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub garage_tag: Option<String>,
    /// One of `Manual`, `Tracker`, `Automatic` (`C-023`). Unlike `status`,
    /// omitted or `null` is accepted -- a vehicle may simply have no tag yet.
    /// Anything else is refused, never defaulted (mirrors HRMS-922).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub garage_tag_origin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<chrono::NaiveDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<chrono::NaiveDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_by: Option<String>,
}
