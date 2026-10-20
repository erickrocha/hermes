use sea_orm::entity::prelude::*;

/// `EPIC-MT-04` (`C-026`, `TRM-261...265`): one time a planned maintenance was not
/// carried out. `business_key` is unique per tenant, so it is recorded once.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "non_execution_occurrence")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub record_id: i64,
    pub maintenance_plan_id: i64,
    pub kind: String,
    pub business_key: String,
    pub window_start: DateTimeUtc,
    pub window_end: DateTimeUtc,
    pub arrived_at: Option<DateTimeUtc>,
    pub departed_at: Option<DateTimeUtc>,
    pub dwell_minutes: Option<i32>,
    pub minutes_in_window: Option<i32>,
    pub justification: Option<String>,
    pub justified_by: Option<String>,
    pub justified_at: Option<DateTimeUtc>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
