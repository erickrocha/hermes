use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "business_plan_tier")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub business_plan_id: i64,
    pub up_to_users: i32,
    pub price_per_user_in_cents: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::business_plan_entity::Entity",
        from = "Column::BusinessPlanId",
        to = "super::business_plan_entity::Column::Id"
    )]
    BusinessPlan,
}

impl Related<super::business_plan_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BusinessPlan.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
