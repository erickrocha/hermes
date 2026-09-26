use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-MT-05-S01` (`HRMS-704`): a tenant's priced-service catalogue
/// entry.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PricedServiceJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<i64>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value_cents: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation: Option<String>,
}
