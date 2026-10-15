use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-MT-02-S01` (`HRMS-705`): a costed posting against a work order
/// (`TRM-614`). `partUuid` is required on write; `workOrderItemUuid` is the
/// pendency this issue resolves, if any. `unitValueCents`/`totalValueCents`/
/// `costSource` are server-derived (`TRM-608`/`609`) and ignored on write.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkOrderPostingJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub part_uuid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_order_item_uuid: Option<String>,
    pub quantity: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_value_cents: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_value_cents: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_source: Option<String>,
}
