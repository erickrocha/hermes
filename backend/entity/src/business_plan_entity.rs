use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "business_plan")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub name: String,
    pub price_in_cents: i64,
    pub available_users: i32,
    pub period_days: i32,
    pub payment_date: Date,
    pub daily_ai_quota: i32,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::business_plan_tier_entity::Entity")]
    BusinessPlanTier,
    #[sea_orm(has_many = "super::tenant_plan_entity::Entity")]
    TenantPlan,
}

impl Related<super::business_plan_tier_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BusinessPlanTier.def()
    }
}

impl Related<super::tenant_plan_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TenantPlan.def()
    }
}

crate::impl_auditable_before_save!(ActiveModel);
