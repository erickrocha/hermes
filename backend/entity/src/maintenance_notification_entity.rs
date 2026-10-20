use sea_orm::entity::prelude::*;

/// `EPIC-MT-06` (`C-026`, `TRM-616`): an in-app notice that a part a work
/// order was waiting for has arrived. `part_name` is copied when the notice
/// is raised; `read_at` is empty while the notice is unread.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "maintenance_notification")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub work_order_id: i64,
    pub part_id: i64,
    pub part_name: String,
    pub quantity: f64,
    pub read_at: Option<DateTimeUtc>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
