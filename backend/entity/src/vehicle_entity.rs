use sea_orm::entity::prelude::*;

/// EPIC-FO-01 (HRMS-920...925, D-20): the vehicle register — the first
/// tenant-owned table added since D-09 made the scoping rule mandatory.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "vehicle")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    /// HRMS-921/D-09. The column is NOT NULL in the migration, so `i64` would
    /// read more honestly here — but `impl_tenant_auditable_before_save!`
    /// stamps the scope through `TenantActiveModel::set_tenant_id(Option<i64>)`,
    /// which requires the `ActiveModel` field to be `Option<i64>`. Matching
    /// `user_entity` keeps the one macro usable by every tenant-owned table;
    /// the NOT NULL constraint is what actually refuses an unowned row.
    pub tenant_id: Option<i64>,
    pub plate: String,
    pub model: String,
    /// HRMS-922/D-23(b): persisted as the name of a
    /// `business::domain::enums::VehicleStatus`, not as a database enum type.
    pub status: String,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
