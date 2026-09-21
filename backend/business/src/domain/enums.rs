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
            _ => Err(format!("Invalid role: {}", value))?,
        }
    }
}

/// EPIC-FO-01-S03 (HRMS-922, D-23(b)): the stated vocabulary a vehicle's
/// status is drawn from, so two depots cannot invent two spellings of the
/// same state. The five values were ratified by the product owner on
/// 2026-09-21; adding a sixth is a decision, not a spelling.
///
/// Persisted as the variant's own name (see `vehicle_entity::Model::status`),
/// deliberately without `DeriveActiveEnum` and without a database enum type,
/// exactly like `Role`.
#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum VehicleStatus {
    #[default]
    Active,
    Maintenance,
    Transit,
    Reserved,
    Inactive,
}

impl Display for VehicleStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            VehicleStatus::Active => write!(f, "Active"),
            VehicleStatus::Maintenance => write!(f, "Maintenance"),
            VehicleStatus::Transit => write!(f, "Transit"),
            VehicleStatus::Reserved => write!(f, "Reserved"),
            VehicleStatus::Inactive => write!(f, "Inactive"),
        }
    }
}

impl FromStr for VehicleStatus {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Active" => Ok(Self::Active),
            "Maintenance" => Ok(Self::Maintenance),
            "Transit" => Ok(Self::Transit),
            "Reserved" => Ok(Self::Reserved),
            "Inactive" => Ok(Self::Inactive),
            _ => Err(format!("Invalid vehicle status: {}", value)),
        }
    }
}