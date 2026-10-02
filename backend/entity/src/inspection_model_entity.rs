use sea_orm::entity::prelude::*;

/// `EPIC-MT-07-S10` (`HRMS-715`, `C-027`): a technical-inspection template
/// (`modelosInspecao`).
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "inspection_model")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub name: String,
    pub generates_work_order: bool,
    pub periodicity_days: Option<i32>,
    pub observation: Option<String>,
    pub active: bool,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
