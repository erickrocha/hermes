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
    pub first_login: bool,
    pub enabled: bool,
    /// Set when the billing worker owns the block; NULL means a human did.
    pub blocked_reason: Option<String>,
    pub tenant_id: Option<i64>,
    pub role: String,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
