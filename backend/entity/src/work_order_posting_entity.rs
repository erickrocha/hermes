use sea_orm::entity::prelude::*;

/// `EPIC-MT-02-S01` (`HRMS-705`, `C-026`): a costed posting against a work
/// order -- `entity-inventory.md` §4 "Lançamentos de OS" (`TRM-609`,
/// `TRM-613…615`, `TRM-688`). Only the fields `TRM-614` itself names are
/// here ("the quantity, the unit value, the total, the cost source and,
/// where it was directed at one, the pendency it belongs to") -- legacy's
/// `categoria`/`tipo_lancamento`/`codigo`/`desconto`/`servico_id` describe a
/// service posting this program has no second producer for yet; grow the
/// vocabulary the day that producer exists, the same discipline
/// `StockMovementType::Issue` and `ExpenseOrigin::Import` already follow.
/// `part_id` is a real FK, never a name to reconcile.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "work_order_posting")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub work_order_id: i64,
    pub work_order_item_id: Option<i64>,
    pub part_id: i64,
    pub quantity: f64,
    pub unit_value_cents: i64,
    pub total_value_cents: i64,
    pub cost_source: String,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
