use sea_orm::entity::prelude::*;

/// `C-032` (`HRMS-851`): a tenant's access profile -- a named set of permissions for one role.
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
    /// The role this profile is for: `TenantUser`, `Driver` or `Mechanic`.
    pub base_role: String,
    pub active: bool,
    /// A JSON array of permission codes from the platform catalogue.
    pub permissions: String,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
