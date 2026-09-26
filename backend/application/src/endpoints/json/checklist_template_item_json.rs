use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-CK-02-S01` (`HRMS-651`): one question of a checklist template.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistTemplateItemJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub description: String,
    /// The "per-item OS flag" -- whether a failing answer generates a work
    /// order (`EPIC-CK-03-S02`, not built yet).
    pub generates_work_order: bool,
}
