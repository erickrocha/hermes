use sea_orm::entity::prelude::*;

/// `EPIC-CK-01-S01` (`HRMS-650`, `C-025`, `AD-041`): the one official
/// odometer writer. Every change to `vehicle.odometer_km` (`C-023`) goes
/// through a row here first -- `entity-inventory.md` §4 "Evolução de KM".
///
/// `source_entity`/`source_entity_id` are deliberately not a foreign key:
/// they are polymorphic by the record's own `origin` (a driver checklist
/// today; a work order once `C-026` exists; a garage service once `C-028`
/// exists). Enforcing a key against a target that changes by row, or that
/// does not exist for several origins yet, is not possible honestly.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "km_evolution")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub km: f64,
    pub recorded_at: DateTimeUtc,
    /// Persisted as a `KmOrigin` name -- `TRM-151`'s seven-value vocabulary,
    /// stated in English already, not invented here.
    pub origin: String,
    pub source_entity: Option<String>,
    pub source_entity_id: Option<i64>,
    pub notes: Option<String>,
    pub recorded_by_user_id: Option<i64>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
