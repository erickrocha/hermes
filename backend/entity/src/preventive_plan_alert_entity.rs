use sea_orm::entity::prelude::*;

/// `EPIC-MT-07-S08` (`HRMS-713`, `C-027`): an intermediate inspection alert on
/// a preventive plan (`TRM-323…325`).
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "preventive_plan_alert")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub preventive_plan_id: i64,
    pub at_km: f64,
    pub title: String,
    pub inspection_model: String,
    pub discharged_cycle: Option<String>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
