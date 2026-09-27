use sea_orm::entity::prelude::*;

/// `EPIC-SP-03-S01` (`HRMS-802`, `C-030`): a request to buy a part --
/// `entity-inventory.md` §4 "Pedidos de compra" (`TRM-640…643`, `651…657`).
/// `part_id` is a real FK, not the legacy `peca_nome` free-text field --
/// hermes has a first-class `part` catalogue now (unlike whenever
/// `pedidosCompra` was first built), so referencing it by identifier is
/// strictly better than matching on a name, the same "no name-matching where
/// an id will do" bias `TRM-656` is fixed by design against (see this
/// Change's plan doc). `work_order_item_id` (the pendency, `TRM-644…650`) is
/// deliberately absent -- it is `EPIC-SP-03-S02`'s own field, added the same
/// day its state-machine wiring is built.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "purchase_order")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub part_id: i64,
    pub quantity: f64,
    pub suggested_supplier: Option<String>,
    pub observation: Option<String>,
    pub status: String,
    pub work_order_id: Option<i64>,
    pub vehicle_id: Option<i64>,
    pub ordered_at: Option<Date>,
    pub expected_delivery_date: Option<Date>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
