use sea_orm::entity::prelude::*;

/// `EPIC-MT-05-S01` (`HRMS-704`, `C-026`): the service-category catalogue a
/// work order's `service_type` will eventually reference --
/// `entity-inventory.md` §4 "tiposServico". `code` is unique per tenant, the
/// same shape every other short-code catalogue in this program (province
/// acronym, city) already uses.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "service_type")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub code: String,
    pub name: String,
    pub category: Option<String>,
    pub active: bool,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
