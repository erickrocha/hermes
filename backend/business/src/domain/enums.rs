use std::fmt::{Display, Formatter};
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum Role {
    SysAdmin,
    TenantOwner,
    #[default]
    TenantUser,
}

impl Display for Role {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::SysAdmin => write!(f, "SysAdmin"),
            Role::TenantOwner => write!(f, "TenantOwner"),
            Role::TenantUser => write!(f, "TenantUser"),
        }
    }
}

impl FromStr for Role {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "SysAdmin" => Ok(Self::SysAdmin),
            "TenantOwner" => Ok(Self::TenantOwner),
            "TenantUser" => Ok(Self::TenantUser),
            _ => Err(format!("Invalid recommended period: {}", value))?,
        }
    }
}