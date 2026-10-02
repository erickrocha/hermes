use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-SC-01-S01` (`HRMS-600`): the customer registry's HTTP shape.
#[derive(Serialize, Deserialize, Debug, Clone, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CustomerJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    /// Only an unbound platform administrator may name it; a tenant owner's
    /// customers are always written into their own tenant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<i64>,
    pub name: String,
    #[serde(default)]
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<chrono::NaiveDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<chrono::NaiveDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_by: Option<String>,
}
