use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "tenant_billing")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub grace_days: Option<i32>,
    pub cycle_started_at: Option<DateTimeUtc>,
    pub pending_business_plan_id: Option<i64>,
    pub suspended_at: Option<DateTimeUtc>,
    pub suspended_by: Option<String>,
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
    #[sea_orm(
        belongs_to = "super::business_plan_entity::Entity",
        from = "Column::PendingBusinessPlanId",
        to = "super::business_plan_entity::Column::Id"
    )]
    PendingBusinessPlan,
}

impl Related<super::tenant_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tenant.def()
    }
}

crate::impl_tenant_auditable_before_save!(ActiveModel);