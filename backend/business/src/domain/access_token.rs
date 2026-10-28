use crate::domain::enums::ProfileScope;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessToken {
    pub access_token: String,
    pub token_type: String,
    pub expire_in: i64,
    pub refresh_token: Option<String>,
    pub email: String,
    pub uuid: String,
    pub name: String,
    pub user_id: i64,
    /// `C-035`: the session's reach and what it may do, so a client draws its menus from the login
    /// answer. The permissions are advisory to the client; the server resolves them again on every call.
    pub scope: ProfileScope,
    pub profile_key: Option<String>,
    pub profile_name: Option<String>,
    pub permissions: Vec<String>,
    pub tenant_id: Option<i64>,
    /// HRMS-204/OBS-TP-05: the session tenant's public identifier. Resolved by
    /// the endpoint after the token is minted; it is deliberately not a JWT
    /// claim, since nothing authorizes on it.
    pub tenant_uuid: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,
    pub uuid: String,
    pub name: String,
    pub user_id: i64,
    pub tenant_id: Option<i64>,
}

/// A wall-display credential is a device principal, never a user account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalType {
    Device,
}

/// Claims for the long-lived, tenant-bound, read-only Android TV credential.
/// Its required marker and device-only fields keep it disjoint from user JWTs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceClaims {
    pub sub: String,
    pub exp: i64,
    pub device_id: i64,
    pub device_uuid: String,
    pub tenant_id: i64,
    pub credential_version: i64,
    pub principal_type: PrincipalType,
}

#[derive(Default)]
pub struct ClaimsBuilder {
    sub: Option<String>,
    exp: Option<i64>,
    uuid: Option<String>,
    name: Option<String>,
    user_id: Option<i64>,
    tenant_id: Option<i64>,
}

impl ClaimsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_sub(mut self, sub: impl Into<String>) -> Self {
        self.sub = Some(sub.into());
        self
    }

    pub fn exp(mut self, exp: i64) -> Self {
        self.exp = Some(exp);
        self
    }

    pub fn uuid(mut self, uuid: impl Into<String>) -> Self {
        self.uuid = Some(uuid.into());
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn user_id(mut self, user_id: i64) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn tenant_id(mut self, tenant_id: Option<i64>) -> Self {
        self.tenant_id = tenant_id;
        self
    }

    pub fn build(self) -> Result<Claims, String> {
        Ok(Claims {
            sub: self.sub.ok_or("sub is required")?,
            exp: self.exp.ok_or("exp is required")?,
            uuid: self.uuid.ok_or("uuid is required")?,
            name: self.name.ok_or("name is required")?,
            user_id: self.user_id.ok_or("user_id is required")?,
            tenant_id: self.tenant_id, // optional, defaults to None
        })
    }
}

impl Claims {
    pub fn builder() -> ClaimsBuilder {
        ClaimsBuilder::new()
    }
}
