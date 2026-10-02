use sea_orm::entity::prelude::*;

/// EPIC-FO-03 (HRMS-931…934, D-23(d)): a driver's responsibility for a
/// vehicle, from `started_at` until `ended_at`. The table also carries a
/// generated `live_vehicle_id` column behind `uq_vehicle_assignment_live`;
/// it is computed by the database and deliberately not mapped here.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "vehicle_assignment")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    /// D-09: `Option` for `impl_tenant_auditable_before_save!`; NOT NULL in
    /// the schema (see `vehicle_entity`).
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub driver_id: i64,
    pub started_at: DateTimeUtc,
    pub ended_at: Option<DateTimeUtc>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
