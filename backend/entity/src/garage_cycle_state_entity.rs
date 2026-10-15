use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "garage_cycle_state")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub reference_odometer_meters: Option<f64>,
    pub last_observed_odometer_meters: Option<f64>,
    pub last_observed_at: Option<DateTimeUtc>,
    pub open_cycle_started_at: Option<DateTimeUtc>,
    pub open_cycle_start_odometer_meters: Option<f64>,
    pub open_cycle_distance_meters: f64,
    pub last_completed_cycle_distance_meters: Option<f64>,
    pub last_completed_cycle_at: Option<DateTimeUtc>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
