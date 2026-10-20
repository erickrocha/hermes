use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// `EPIC-MT-06` (`C-026`, `TRM-616`): "a part this work order was waiting for has arrived".
/// `partName` is the name copied when the notice was raised.
#[derive(Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceNotificationJson {
    pub uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_order_uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_uuid: Option<String>,
    pub part_name: String,
    pub quantity: f64,
    pub read: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_at: Option<NaiveDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<NaiveDateTime>,
}

#[derive(Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UnreadCountJson {
    pub count: u64,
}

#[derive(Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarkedReadJson {
    pub marked: u64,
}

/// `?unread=true` keeps only the notices not read yet; absent or `false` lists them all.
#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct UnreadQuery {
    pub unread: Option<bool>,
}
