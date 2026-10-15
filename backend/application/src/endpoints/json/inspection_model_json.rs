use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `EPIC-MT-07-S10` (`HRMS-715`): a technical-inspection template. `items` are
/// the descriptions, in order. `generatesWorkOrder` defaults to `true`.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectionModelJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<i64>,
    pub name: String,
    #[serde(default = "yes")]
    pub generates_work_order: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub periodicity_days: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation: Option<String>,
    #[serde(default = "yes")]
    pub active: bool,
    pub items: Vec<String>,
}

fn yes() -> bool {
    true
}
