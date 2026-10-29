use sea_orm::entity::prelude::*;

/// `C-036` (`HRMS-877`): a request one tenant made inside a descendant of its own. Append-only. It belongs to two
/// tenants, so it carries no `tenant_id` and the tenant scope does not apply: the service reads it by `affected` or
/// `actor` tenant explicitly.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "cross_tenant_action")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub actor_user_id: i32,
    pub actor_email: String,
    pub actor_tenant_id: i32,
    pub affected_tenant_id: i32,
    pub kind: String,
    pub method: String,
    pub route: String,
    pub path: String,
    pub occurred_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
