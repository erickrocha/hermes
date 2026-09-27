use sea_orm::entity::prelude::*;

/// `EPIC-SP-02-S01` (`HRMS-801`, `C-030`): a part's stock ledger --
/// `entity-inventory.md` §4 "Movimentações de estoque" (`TRM-602…612`,
/// `617`). No `deleted`/soft-delete column: `TRM-607` ("exclude deleted
/// movements from a part's stock") is a legacy blob concern -- a hard-deleted
/// row here is already gone from every `SUM`, nothing to exclude.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "stock_movement")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub part_id: i64,
    pub movement_type: String,
    pub quantity: f64,
    pub unit_value_cents: Option<i64>,
    pub total_value_cents: Option<i64>,
    pub cost_source: String,
    pub supplier: Option<String>,
    pub invoice_number: Option<String>,
    pub entry_date: Option<Date>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
