use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `C-032` (`HRMS-851`): a tenant's access profile.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccessProfileJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<i64>,
    pub name: String,
    /// `TenantUser`, `Driver` or `Mechanic`.
    pub base_role: String,
    #[serde(default = "yes")]
    pub active: bool,
    /// Permission codes from `GET /access-profile/catalogue`.
    #[serde(default)]
    pub permissions: Vec<String>,
}

fn yes() -> bool {
    true
}

/// One grantable permission of the platform catalogue (`HRMS-850`).
#[derive(Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PermissionJson {
    pub code: String,
    pub module: String,
    /// May be granted over the user's own records only.
    pub own: bool,
    /// The roles whose built-in default profile holds it today.
    pub default_for: Vec<String>,
}

/// What the caller may do (`HRMS-853`) -- what the apps and the console draw their menus from.
#[derive(Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EffectivePermissionsJson {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_name: Option<String>,
    pub permissions: Vec<String>,
}

#[derive(Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssignProfileJson {
    /// The profile to give; omitted or null returns the user to their role's default.
    #[serde(default)]
    pub profile_uuid: Option<String>,
}
