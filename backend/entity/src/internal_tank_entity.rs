use sea_orm::entity::prelude::*;

/// `EPIC-FU-07-S01` (`HRMS-943`, `C-029`): the operator's own diesel tank --
/// `entity-inventory.md` §4 "Tanque interno" (`TRM-1540`, `TRM-1544`,
/// `TRM-1545`). A scalar per tenant, not a ledger -- `uq_internal_tank_tenant`
/// enforces at most one row. Only the register half is here: `TRM-1542`'s
/// current-stock computation reads fuellings whose `origin` is the
/// provider's own posting (`TRM-1541`), which `EPIC-FU-02` is the first
/// producer of and does not exist yet; the weekday-profile projection
/// (`TRM-1546…1550`) is its own epic once that exists. `alert_level_liters`/
/// `reserve_level_liters` are captured now so that projection has something
/// to compare against the day it exists, the same "capture the config ahead
/// of its consumer" move `Part.minimum_stock` already made.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "internal_tank")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub capacity_liters: f64,
    pub reference_stock_liters: f64,
    pub reference_at: DateTimeUtc,
    pub alert_level_liters: f64,
    pub reserve_level_liters: f64,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
