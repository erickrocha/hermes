use sea_orm::entity::prelude::*;

/// `EPIC-CK-03-S01` (`HRMS-652`, `C-025`): a submitted driver checklist --
/// `entity-inventory.md` §5 "Checklists do motorista". A `Departure` run
/// opens a vehicle's possession cycle; a `Return` closes the one it names
/// in `opening_checklist_id`; a `Standalone` run has no possession side
/// effect at all (`TRM-100`/`TRM-103`/`TRM-104` only ever mention departure
/// and return for that). Vehicle possession is derived from these rows,
/// never cached as a field on `vehicle` -- see this Change's own plan doc,
/// "What this plan deliberately does not cover".
///
/// `odometer_km` is not a second odometer record: every run also writes a
/// `km_evolution` row (origin `DriverChecklist`) through the one official
/// writer `EPIC-CK-01-S01` built (`AD-041`).
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "checklist_run")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub checklist_template_id: i64,
    pub driver_id: i64,
    pub vehicle_id: i64,
    /// Persisted as a `ChecklistType` name -- the same three-value
    /// vocabulary `checklist_template.checklist_type` uses.
    pub checklist_type: String,
    pub odometer_km: f64,
    pub notes: Option<String>,
    /// Set only on a `Return` run: the `Departure` run it closes
    /// (`TRM-104`). Unique when present, so at most one `Return` may ever
    /// close a given `Departure` (`1 = 0` otherwise cannot happen, but the
    /// DB enforces it too rather than trusting the use case alone).
    pub opening_checklist_id: Option<i64>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
