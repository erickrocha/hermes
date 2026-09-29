use sea_orm::entity::prelude::*;

/// `EPIC-FU-01-S01`/`EPIC-FU-02-S01` (`HRMS-942`/`944`, `C-029`): a vehicle's
/// fuelling -- `entity-inventory.md` §4 "Abastecimentos" (`TRM-500…599`,
/// `TRM-1500…1555`). `provider_transaction_id` (`TRM-511`, added by
/// `EPIC-FU-02-S01`) is `None` for a manual entry; no column for "the
/// provider's own vehicle reference" -- it is only ever used to *match* a
/// vehicle at import time (`TRM-507`), never read again once `vehicle_id`, a
/// real FK, is set. No `deleted_at`: soft delete exists in legacy to let
/// reconciliation supersede a record without losing it (`EPIC-FU-04`), and
/// nothing reconciles yet.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "fuel_entry")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub recorded_at: DateTimeUtc,
    pub volume_liters: f64,
    pub value_cents: i64,
    pub odometer_km: Option<f64>,
    pub station: Option<String>,
    pub full_tank: bool,
    pub origin: String,
    pub provider_transaction_id: Option<String>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
