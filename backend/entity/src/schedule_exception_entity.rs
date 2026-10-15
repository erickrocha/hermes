use sea_orm::entity::prelude::*;

/// `EPIC-SC-02-S04` (`HRMS-606`, `C-024`): a day exception against a demand
/// -- cancellation, de-allocation or substitution -- so today's deviation is
/// data instead of a note to remember (`entity-inventory.md` §2 "Exceções de
/// escala").
///
/// `extra_trip_id` names the `viagensExtra` row a substitution opens against
/// -- `EPIC-SC-03`, not yet built (it needs its own design pass, per
/// `scheduling_implementation_plan.md`). The column exists because
/// `entity-inventory.md` states it as part of this entity's real shape;
/// there is deliberately **no foreign key** on it yet, because there is no
/// table to reference. Add the constraint in the same migration that builds
/// `EPIC-SC-03`'s trip table -- do not invent one now against a table that
/// does not exist.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "schedule_exception")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub demand_id: i64,
    pub date: Date,
    /// Persisted as a `ScheduleExceptionType` name, the same shape
    /// `vehicle.status` uses -- unlike most free-text fields in this
    /// domain, the story itself names the vocabulary (cancellation /
    /// de-allocation / substitution), so it is not invented here.
    pub exception_type: String,
    pub new_driver_id: Option<i64>,
    pub new_vehicle_id: Option<i64>,
    pub reason: Option<String>,
    pub extra_trip_id: Option<i64>,
    pub status: Option<String>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
