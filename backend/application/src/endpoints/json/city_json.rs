use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CityJson {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub province_id: i64,
    pub name: String,
}
