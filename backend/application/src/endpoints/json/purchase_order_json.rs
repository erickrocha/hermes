use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-SP-03-S01` (`HRMS-802`): a request to buy a part. `number`
/// (`PO-{id}`) and `status` are server-derived; `status` always starts
/// `Requested` regardless of what the caller sends.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseOrderJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<i64>,
    pub part_uuid: String,
    pub quantity: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_supplier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_order_uuid: Option<String>,
    /// `TRM-644`: the pendency this order was raised from, when there was
    /// one. Absent (with `workOrderUuid` present) creates a synthetic
    /// placeholder pendency instead (`TRM-645`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_order_item_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vehicle_uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordered_at: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_delivery_date: Option<NaiveDate>,
}

/// `EPIC-SP-03-S01` (`TRM-651`): the body of the `mark-ordered` transition.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarkOrderedJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordered_at: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_delivery_date: Option<NaiveDate>,
}
