use sea_orm::entity::prelude::*;

/// `EPIC-SP-01-S01` (`HRMS-800`, `C-030`): a tenant's parts catalogue --
/// `entity-inventory.md` §4 "Peças". `TRM-600` lists "current stock" as a
/// catalogue field, but `TRM-602` requires it be *derived* by summing the
/// stock-movement ledger, never incremented on a stored figure -- so, unlike
/// the legacy `estoque_atual` column, there is no stock column here at all.
/// This is the same "fixed by design" move as the work order's `OS-{id}`
/// number (`maintenance-work-orders_implementation_plan.md`): the entire
/// class of reconciliation defects `TRM-604`/`605`/`617` exist to patch
/// (an unloaded ledger, a stored figure drifting from its ledger) has no
/// stored figure to drift from in the first place. `moving_average_cost_cents`
/// and `last_purchase_price_cents` are themselves cached, mutated by
/// `EPIC-SP-02`'s stock-entry use case, not summed on every read (`TRM-608`,
/// `TRM-610`) -- only *stock* is ledger-derived, per `TRM-602`'s own wording.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "part")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub name: String,
    pub category: Option<String>,
    pub application: Option<String>,
    pub minimum_stock: f64,
    pub unit: String,
    pub unit_value_cents: Option<i64>,
    pub default_supplier: Option<String>,
    pub location: Option<String>,
    pub observation: Option<String>,
    pub moving_average_cost_cents: Option<i64>,
    pub last_purchase_price_cents: Option<i64>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
