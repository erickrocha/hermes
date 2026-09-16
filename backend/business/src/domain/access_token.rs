use serde::{Deserialize, Serialize};
use crate::domain::enums::Role;

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
    pub role: Role,
    pub tenant_id: Option<i64>,
    pub first_login: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,
    pub uuid: String,
    pub name: String,
    pub user_id: i64,
    pub role: Role,
    pub tenant_id: Option<i64>,
}

#[derive(Default)]
pub struct ClaimsBuilder {
    sub: Option<String>,
    exp: Option<i64>,
    uuid: Option<String>,
    name: Option<String>,
    user_id: Option<i64>,
    role: Option<Role>,
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

    pub fn role(mut self, role: Role) -> Self {
        self.role = Some(role);
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
            role: self.role.ok_or("role is required")?,
            tenant_id: self.tenant_id, // optional, defaults to None
        })
    }
}

impl Claims {
    pub fn builder() -> ClaimsBuilder {
        ClaimsBuilder::new()
    }
}
