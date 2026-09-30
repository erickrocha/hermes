use sea_orm::entity::prelude::*;

/// `EPIC-GA-02-S01` (`HRMS-958`): a triage -- the record that a vehicle is at base for services (`TRM-410`).
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "garage_attendance")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub attendance_date: Date,
    pub checked_in_at: DateTimeUtc,
    pub status: String,
    pub manual_priority: Option<i32>,
    pub released_at: Option<DateTimeUtc>,
    pub origin: String,
    pub active_marker: Option<i32>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
