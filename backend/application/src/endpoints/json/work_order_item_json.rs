use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-MT-01-S02` (`HRMS-701`): one pendency of a work order.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkOrderItemJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_type: Option<String>,
    /// One of `Pending`, `Resolved`, `Cancelled`, `AwaitingParts`. Defaults
    /// to `Pending` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<chrono::NaiveDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution_description: Option<String>,
    /// `EPIC-SP-03-S02` (`TRM-645`): true when this item is a synthetic
    /// placeholder standing in for a purchase order raised with no named
    /// pendency, rather than a real repair pendency. Read-only.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_purchase_placeholder: bool,
    /// `EPIC-MT-07-S04` (`TRM-310`): the preventive plan this item services,
    /// which must be a plan of the same vehicle as the work order.
    /// Write-only: accepted on create, not echoed back on reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preventive_plan_uuid: Option<String>,
}
