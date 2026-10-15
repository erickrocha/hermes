use sea_orm::entity::prelude::*;

/// `HRMS-512`/`HRMS-515` (`D-16`): a wall display is a tenant-bound device,
/// not a user role. Pairing codes are short-lived and cleared on redemption.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "garage_device")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub name: String,
    pub credential_version: i64,
    #[sea_orm(unique)]
    pub pairing_code: Option<String>,
    pub pairing_code_expires_at: Option<DateTimeUtc>,
    pub paired_at: Option<DateTimeUtc>,
    pub revoked_at: Option<DateTimeUtc>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
