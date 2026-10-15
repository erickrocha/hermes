use sea_orm::entity::prelude::*;

/// `EPIC-MT-07-S09` (`HRMS-714`, `C-027`): a technical inspection of a vehicle
/// (`TRM-333`). Responsible party and instant are the audit columns.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "technical_inspection")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub inspection_model: String,
    pub inspected_at: Date,
    pub odometer_km: f64,
    pub observation: Option<String>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
