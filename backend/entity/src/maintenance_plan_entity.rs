use sea_orm::entity::prelude::*;

/// `EPIC-MT-03-S01` (`HRMS-703`, `C-026`): a planned maintenance window for
/// a vehicle -- `entity-inventory.md` §4 "Manutenções planejadas". The work
/// orders it covers are the other side of `work_order.maintenance_plan_id`,
/// not an `os_id`/`os_ids[]` pair -- legacy's own duplication between a
/// singular and a plural field is not migrated; see this Change's plan doc.
///
/// `priority` and the "forced/authorised" flags `entity-inventory.md` also
/// lists are deliberately absent: no requirement anywhere in the project
/// truth governs their semantics, and inventing behaviour for an
/// unconfirmed field is exactly the discipline this program avoids.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "maintenance_plan")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub date: Date,
    pub planned_start: Option<DateTimeUtc>,
    pub planned_end: Option<DateTimeUtc>,
    pub status: String,
    /// `TRM-236`'s own computation (excluding non-maintenance occupancy) is
    /// not implemented yet -- this is a caller-supplied flag until it is.
    pub affects_schedule: bool,
    pub origin: String,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
