use sea_orm::entity::prelude::*;

/// `EPIC-MT-05-S01` (`HRMS-704`, `C-026`): the priced-service catalogue a
/// costed posting (`EPIC-MT-02`, blocked on `C-030`) will eventually
/// reference -- `entity-inventory.md` §4 "servicosExecutados". No stated
/// uniqueness key, unlike `service_type`'s `code`.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "priced_service")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub name: String,
    pub category: Option<String>,
    pub default_value_cents: Option<i64>,
    pub observation: Option<String>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
