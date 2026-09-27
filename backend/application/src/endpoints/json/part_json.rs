use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-SP-01-S01` (`HRMS-800`): a tenant's parts catalogue entry. `code` is
/// server-derived (`PC-{id}`, the same "fixed by design" move `work_order`'s
/// `OS-{id}` made) and ignored on write.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PartJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<i64>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application: Option<String>,
    #[serde(default)]
    pub minimum_stock: f64,
    pub unit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_value_cents: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_supplier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moving_average_cost_cents: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_purchase_price_cents: Option<i64>,
    /// `EPIC-SP-02-S01` (`TRM-602`): summed from the stock ledger on every
    /// read, never stored. Ignored on write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_stock: Option<f64>,
}
