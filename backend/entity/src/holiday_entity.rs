use sea_orm::entity::prelude::*;

/// `EPIC-SC-01-S03` (`HRMS-602`, `C-024`): a tenant-configured holiday, so
/// scheduling can suppress a recurring line on a day the tenant does not
/// operate (`entity-inventory.md` §1 "Feriados"; `PD-016`: numbers/calendars
/// are tenant configuration, not a hard-coded platform list).
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "holiday")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub date: Date,
    pub name: String,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
