use sea_orm::entity::prelude::*;

/// `EPIC-SC-02-S01` (`HRMS-603`, `C-024`): a unit of recurring transport
/// demand -- a line, an extra line, or a one-off trip type
/// (`entity-inventory.md` §2 "Necessidades"). `days_of_week` and
/// `specific_date` are alternatives: a recurring demand states the former,
/// a one-off demand the latter -- neither is enforced exclusive at the
/// schema level (no stated rule says it must be), so both are nullable.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "transport_demand")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    /// Free text (`tipo_necessidade`): no stated vocabulary, same reasoning
    /// `vehicle.vehicle_type` (`C-023`) used.
    pub demand_type: String,
    pub customer_id: Option<i64>,
    pub line_name: Option<String>,
    pub shift_start: Option<Time>,
    pub shift_end: Option<Time>,
    /// Free-form (e.g. `"MON,WED,FRI"`): no stated encoding in
    /// `entity-inventory.md`, and inventing a bitmask would be a schema
    /// decision this story does not have grounds to make.
    pub days_of_week: Option<String>,
    pub specific_date: Option<Date>,
    pub priority: Option<i32>,
    pub preferred_vehicle_type: Option<String>,
    pub preferred_vehicle_model: Option<String>,
    pub specific_driver_id: Option<i64>,
    pub specific_vehicle_id: Option<i64>,
    pub active: bool,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
