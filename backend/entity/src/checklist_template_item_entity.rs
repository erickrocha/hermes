use sea_orm::entity::prelude::*;

/// `EPIC-CK-02-S01` (`HRMS-651`, `C-025`): one question of a
/// `checklist_template`, in the order it was added (`id` order -- no
/// separate position column for a list this short-lived to reorder).
/// `generates_work_order` is the "per-item OS flag" `entity-inventory.md`
/// §5 names on `checklistModelos.itens`.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "checklist_template_item")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub checklist_template_id: i64,
    pub description: String,
    pub generates_work_order: bool,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
