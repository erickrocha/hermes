use sea_orm::entity::prelude::*;

/// `EPIC-MT-04` (`C-026`, `TRM-266`): the one history record of a work order's
/// non-executions. Never deleted; it accumulates occurrences.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "non_execution_record")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    #[sea_orm(unique)]
    pub work_order_id: i64,
    pub vehicle_id: i64,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
