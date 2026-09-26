use crate::endpoints::json::checklist_template_item_json::ChecklistTemplateItemJson;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-CK-02-S01` (`HRMS-651`): a reusable driver-checklist template's
/// HTTP shape. `items` is always populated on create and on a single-template
/// read; a list-page row leaves it empty rather than joining every template's
/// items into a page nobody asked to see in that shape.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistTemplateJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<i64>,
    pub name: String,
    /// One of `Departure`, `Return`, `Standalone`.
    pub checklist_type: String,
    pub active: bool,
    #[serde(default)]
    pub items: Vec<ChecklistTemplateItemJson>,
}
