use sea_orm::entity::prelude::*;

/// `EPIC-GA-01-S01` (`HRMS-956`): a garage service the tenant's yard performs.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "garage_service_model")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub name: String,
    /// `TRM-433`: the normalised name, unique per tenant.
    pub name_key: String,
    pub display_order: i32,
    pub active: bool,
    pub service_group: String,
    /// `TRM-470`: an explicit attribute, never a name match.
    pub required_for_departure: bool,
    /// `TRM-462`: governed by the tank level, not by elapsed time (an explicit attribute).
    pub governed_by_tank: bool,
    pub applicability: String,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
