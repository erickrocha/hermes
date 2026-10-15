use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-CK-04-S01`/`EPIC-CK-03-S02` (`HRMS-654`/`653`): a driver's answer
/// to one template item. `work_order_uuid` is populated only when this
/// flagged, non-conforming answer actually opened a work order.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistAnswerJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub checklist_template_item_uuid: String,
    /// One of `Conforming`, `NonConforming` (`TRM-115`/`TRM-116`).
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_order_uuid: Option<String>,
}
