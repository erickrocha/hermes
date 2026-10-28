use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "user")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub name: Option<String>,
    #[sea_orm(unique)]
    pub email: String,
    pub password: String,
    pub enabled: bool,
    /// Set when the billing worker owns the block; NULL means a human did.
    pub blocked_reason: Option<String>,
    pub tenant_id: Option<i64>,
    /// `C-035`: the access profile held -- a system profile or one of the tenant's own.
    pub profile_id: i64,
    /// `C-035`: a JSON array of grants held on top of the profile's.
    pub extra_permissions: String,
    /// `C-032` (`HRMS-861`): the person's CPF, digits only; unique per tenant when present.
    pub cpf: Option<String>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
