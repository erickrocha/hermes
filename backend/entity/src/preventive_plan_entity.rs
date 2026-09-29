use sea_orm::entity::prelude::*;

/// `EPIC-MT-07-S01` (`HRMS-706`, `C-027`): a vehicle's preventive-maintenance
/// plan -- `entity-inventory.md` §5 "Preventivas" (`TRM-300…305`). Only the
/// plan's own declared facts are stored; `next_service_km`/`next_service_date`
/// are never columns here -- they are `last_service_* + interval_*`, derived
/// at read time the same way `TRM-602` already made stock a derived figure,
/// never a stored one that can drift from it. No `modelo_preventiva_id` FK:
/// the plan-template entity (`TRM-323…325`'s intermediate alerts) is a BLOB
/// in legacy with no hermes producer yet -- `plan_name` is free text until
/// that catalogue exists, the same "free text before a catalogue" shape
/// `work_order.service_type` already uses.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "preventive_plan")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub plan_name: String,
    pub control_type: String,
    pub interval_km: Option<f64>,
    pub interval_days: Option<i32>,
    pub last_service_km: Option<f64>,
    pub last_service_date: Option<Date>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
