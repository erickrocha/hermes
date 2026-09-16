use business::domain::enums::Role;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccessTokenJson {
    pub access_token: String,
    pub token_type: String,
    pub expire_in: i64,
    pub refresh_token: Option<String>,
    pub email: String,
    pub uuid: String,
    pub name: String,
    pub user_id: i64,
    pub role: Role,
    pub tenant_id: Option<i64>,
    pub first_login: bool,
}
