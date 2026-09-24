use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProvinceJson {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub acronym: String,
    pub name: String,
    pub country_code: String,
}

