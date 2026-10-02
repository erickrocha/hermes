use sea_orm::entity::prelude::*;

/// `EPIC-SC-02-S03` (`HRMS-605`, `C-024`): a manual entry for one day of a
/// demand's schedule, so a one-off adjustment does not require editing the
/// recurring allocation (`entity-inventory.md` §2 "Escala do dia"). Unlike
/// `transport_demand_allocation` (a date *range*), this is a single day --
/// at most one entry per demand per date (`uq_daily_schedule_demand_date`).
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "daily_schedule")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub demand_id: i64,
    pub driver_id: i64,
    pub vehicle_id: i64,
    pub date: Date,
    pub start_time: Option<Time>,
    pub end_time: Option<Time>,
    pub notes: Option<String>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
