use sea_orm::entity::prelude::*;

/// `EPIC-CK-02-S01` (`HRMS-651`, `C-025`): a reusable driver-checklist
/// template -- `entity-inventory.md` §5 "Modelos de checklist". Its items
/// live in `checklist_template_item`, one row each, so a future checklist
/// answer (`EPIC-CK-04`) can reference the exact item it answers.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "checklist_template")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub name: String,
    /// Persisted as a `ChecklistType` name -- `entity-inventory.md`'s own
    /// wording ("departure / return / standalone"), not invented here.
    pub checklist_type: String,
    pub active: bool,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
