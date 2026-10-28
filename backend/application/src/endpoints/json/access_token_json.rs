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
    /// `C-035`: `Platform` or `Tenant` -- the session's reach.
    pub scope: String,
    /// The system profile's key (`SysAdmin`, `Driver`, ...) when one is held.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_name: Option<String>,
    /// The session's effective grants (`code` or `code@own`) -- what a client draws its menus from. The
    /// server resolves them again on every call, so this is advisory.
    #[serde(default)]
    pub permissions: Vec<String>,
    pub tenant_id: Option<i64>,
    /// HRMS-204/OBS-TP-05: tenants are addressed by their public uuid, so the
    /// console needs the session tenant's uuid to fetch it directly. The
    /// numeric `tenant_id` is retained for authorization scoping only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_uuid: Option<String>,
}
