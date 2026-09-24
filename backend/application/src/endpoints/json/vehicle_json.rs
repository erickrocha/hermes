use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// EPIC-FO-01-S05 (HRMS-924): the vehicle register's HTTP shape.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<chrono::NaiveDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<chrono::NaiveDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_by: Option<String>,
}
