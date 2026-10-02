use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-SP-04-S01` (`HRMS-803`): a direct vehicle expense -- toll, plan fee
/// or parking. `origin` is server-derived (`Manual` on this endpoint) and
/// ignored on write.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VehicleExpenseJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<i64>,
    pub vehicle_uuid: String,
    pub category: String,
    pub competence_period: String,
    pub issue_date: NaiveDate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supplier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invoice_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub value_cents: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}
