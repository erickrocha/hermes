use sea_orm::entity::prelude::*;

/// `EPIC-FU-06-S03` (`HRMS-953`): a tenant's tank-gauge thresholds.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "fuel_gauge_setting")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub suspect_margin_ratio: f64,
    pub set_aside_min_expected_liters: f64,
    pub set_aside_ratio: f64,
    pub large_fuelling_ratio: f64,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
