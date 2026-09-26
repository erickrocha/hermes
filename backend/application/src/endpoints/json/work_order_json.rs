use crate::endpoints::json::work_order_item_json::WorkOrderItemJson;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-MT-01-S01` (`HRMS-700`): a maintenance work order's HTTP shape.
/// `number` is the display form of the row's own id (`OS-{id}`) -- see this
/// Change's plan doc for why there is no separate counter. `origin` and
/// `status` are server-controlled on create; the client never sends them.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkOrderJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
    pub vehicle_uuid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opened_at: Option<chrono::NaiveDateTime>,
    pub odometer_km: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_type: Option<String>,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub responsible: Option<String>,
    /// One of `Open`, `PartiallyResolved`, `AwaitingParts`, `Concluded`,
    /// `Cancelled`. Always `Open` on a freshly created work order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation: Option<String>,
    #[serde(default)]
    pub external_service: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supplier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invoice_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invoice_value_cents: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invoice_date: Option<chrono::NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concluded_at: Option<chrono::NaiveDate>,
    /// Populated on create and on a single-work-order read; a list-page row
    /// leaves it empty, the same convention `ChecklistTemplateJson` uses.
    #[serde(default)]
    pub items: Vec<WorkOrderItemJson>,
}
