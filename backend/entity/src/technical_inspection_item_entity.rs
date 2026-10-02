use sea_orm::entity::prelude::*;

/// `EPIC-MT-07-S09` (`HRMS-714`, `C-027`): one answer of a technical
/// inspection. A non-conforming one carries the work-order item it matched or
/// opened (`TRM-333`).
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "technical_inspection_item")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub technical_inspection_id: i64,
    pub description: String,
    pub conforming: bool,
    pub observation: Option<String>,
    pub work_order_item_id: Option<i64>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
