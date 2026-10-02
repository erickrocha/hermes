use sea_orm::entity::prelude::*;

/// `EPIC-SP-04-S01` (`HRMS-803`, `C-030`): a direct vehicle expense -- toll,
/// plan fee or parking -- `entity-inventory.md` §4 "Despesas por veículo"
/// (`TRM-660`, `TRM-661`). `vehicle_id` is a real FK; the legacy entity's own
/// `plate` field is not stored here, since it is always derivable by joining
/// `vehicle` -- the same "no free-text duplicate of a real FK" bias
/// `purchase_order.part_id` already established over `pedidosCompra`'s own
/// `peca_nome`. No attached-invoice-file column: hermes has no file-storage
/// mechanism anywhere yet, the same gap that kept `TRM-632`'s attachment off
/// `stock_movement`.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "vehicle_expense")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub category: String,
    pub competence_period: String,
    pub issue_date: Date,
    pub supplier: Option<String>,
    pub invoice_number: Option<String>,
    pub description: Option<String>,
    pub value_cents: i64,
    pub origin: String,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
