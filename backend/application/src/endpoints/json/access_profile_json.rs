use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `C-032`/`C-035` (`HRMS-851`): an access profile -- a system one (read-only, every tenant sees it) or
/// a tenant's own.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccessProfileJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    /// The owning tenant; `null` for a system or platform profile. The platform names it on create.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<i64>,
    pub name: String,
    /// Read-only: seeded by the platform, never edited.
    #[serde(default)]
    pub system: bool,
    /// Read-only: the system profile's key (`SystemAdministrator`, `TenantOwner`, `TenantUser`, `Driver`, `Mechanic`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default = "yes")]
    pub active: bool,
    /// Grants from `GET /access-profile/catalogue`: `code`, or `code@own` for a permission with `own`.
    #[serde(default)]
    pub permissions: Vec<String>,
}

fn yes() -> bool {
    true
}

/// One permission of the platform catalogue (`HRMS-850`) the caller may grant.
#[derive(Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PermissionJson {
    pub code: String,
    pub module: String,
    /// `Tenant` or `Platform`: which profiles may hold it.
    pub scope: String,
    /// May be granted over the user's own records only, as `code@own`.
    pub own: bool,
    /// The system profiles below the owner that hold it.
    pub default_for: Vec<String>,
}

/// What the caller may do (`HRMS-853`) -- what the apps and the console draw their menus from.
#[derive(Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EffectivePermissionsJson {
    /// `Platform` or `Tenant`.
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_name: Option<String>,
    /// The system profile's key when one is held.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_key: Option<String>,
    /// Grants held on top of the profile's.
    pub extra_permissions: Vec<String>,
    /// Effective grants: profile, extras and baseline. `code@own` marks an own-records grant.
    pub permissions: Vec<String>,
}

#[derive(Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssignProfileJson {
    /// The profile to give.
    pub profile_uuid: String,
    /// Grants to hold on top of the profile's; replaces the current extras.
    #[serde(default)]
    pub extra_permissions: Vec<String>,
}
