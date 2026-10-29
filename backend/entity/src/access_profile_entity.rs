use sea_orm::entity::prelude::*;

/// `C-032`/`C-035` (`HRMS-851`): an access profile -- a named set of grants. `tenant_id` is NULL for the
/// system profiles, which every tenant sees.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "access_profile")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub name: String,
    /// The normalised name, unique per tenant.
    pub name_key: String,
    /// `Platform` or `Tenant`: where the profile -- and its holders -- live.
    /// Seeded by the platform, read-only, permissions re-derived from the catalogue at boot.
    pub system: bool,
    /// The system profile's key (`SysAdmin`, `TenantOwner`, `TenantUser`, `Driver`, `Mechanic`); NULL otherwise.
    pub key: Option<String>,
    pub active: bool,
    /// A JSON array of grants (`code` or `code@own`) from the platform catalogue.
    pub permissions: String,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
