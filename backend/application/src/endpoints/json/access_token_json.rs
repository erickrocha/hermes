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
    /// HRMS-204/OBS-TP-05: tenants are addressed by their public uuid, so the
    /// console needs the session tenant's uuid to fetch it directly. The
    /// numeric `tenant_id` is retained for authorization scoping only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_uuid: Option<String>,
}
