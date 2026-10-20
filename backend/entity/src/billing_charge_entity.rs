use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "billing_charge")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub tenant_uuid: Vec<u8>,
    pub period_start: DateTimeUtc,
    pub period_end: DateTimeUtc,
    pub tenant_name: String,
    pub plan_name: String,
    pub currency: String,
    pub price_in_cents: i64,
    pub price_per_extra_user_in_cents: i64,
    pub included_users: i32,
    pub active_users: i32,
    pub extra_users: i32,
    pub grace_days: i32,
    pub total_in_cents: i64,
    pub status: String,
    pub paid_at: Option<DateTimeUtc>,
    pub paid_by: Option<String>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::tenant_entity::Entity",
        from = "Column::TenantId",
        to = "super::tenant_entity::Column::Id"
    )]
    Tenant,
}

impl Related<super::tenant_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tenant.def()
    }
}

crate::impl_tenant_auditable_before_save!(ActiveModel);