use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-SP-02-S01` (`HRMS-801`): one stock-ledger entry. On a `POST .../entry`
/// request, `movement_type` and `cost_source` are ignored -- both are
/// server-derived.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StockMovementJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub movement_type: Option<String>,
    pub quantity: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_value_cents: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_value_cents: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supplier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invoice_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_date: Option<NaiveDate>,
}

/// `EPIC-SP-02-S01` (`TRM-617`): the target balance an adjustment reconciles
/// the part's ledger-derived stock to.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StockAdjustmentJson {
    pub new_balance: f64,
}
