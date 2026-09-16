use chrono::NaiveDate;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "tenant_plan")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: i64,
    pub business_plan_id: i64,
    pub payment_date: NaiveDate,
    pub active: bool,
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
        from = "Column::BusinessPlanId",
        to = "super::business_plan_entity::Column::Id"
    )]
    BusinessPlan,
}

impl Related<super::tenant_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tenant.def()
    }
}

impl Related<super::business_plan_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BusinessPlan.def()
    }
}

crate::impl_auditable_before_save!(ActiveModel);

