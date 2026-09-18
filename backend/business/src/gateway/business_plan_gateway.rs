use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::domain::business_plan::{BusinessPlan, BusinessPlanEntityMapper};
use entity::business_plan_entity::Model as BusinessPlanEntity;
use entity::{business_plan_entity, tenant_entity};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, EntityTrait, QueryFilter, QueryOrder,
    TryIntoModel,
};

pub struct BusinessPlanGateway {
    db: DbConn,
}

impl BusinessPlanGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    pub async fn save(&self, plan: BusinessPlan) -> Result<BusinessPlanEntity, DbErr> {
        BusinessPlanEntityMapper::build_active_model(plan)
            .save(&self.db)
            .await?
            .try_into_model()
    }

    pub async fn find_by_id(&self, id: i64) -> Result<Option<business_plan_entity::Model>, DbErr> {
        business_plan_entity::Entity::find_by_id(id).one(&self.db).await
    }

    pub async fn find_by_uuid(&self, uuid: &str) -> Result<Option<business_plan_entity::Model>, DbErr> {
        business_plan_entity::Entity::find()
            .filter(business_plan_entity::Column::Uuid.eq(string_to_bytes(uuid)))
            .one(&self.db)
            .await
    }

    pub async fn find_all(&self) -> Result<Vec<business_plan_entity::Model>, DbErr> {
        business_plan_entity::Entity::find()
            .order_by_desc(business_plan_entity::Column::Id)
            .all(&self.db)
            .await
    }

    /// Remove o plano do catálogo. Recusa se algum tenant ainda o referencia.
    pub async fn delete(&self, id: i64) -> Result<bool, DbErr> {
        if business_plan_entity::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .is_none()
        {
            return Ok(false);
        }

        let in_use = tenant_entity::Entity::find()
            .filter(tenant_entity::Column::BusinessPlanId.eq(id))
            .one(&self.db)
            .await?
            .is_some();
        if in_use {
            return Err(DbErr::Custom(
                "Business plan is still assigned to one or more tenants".to_string(),
            ));
        }

        let result = business_plan_entity::Entity::delete_by_id(id)
            .exec(&self.db)
            .await?;
        Ok(result.rows_affected == 1)
    }
}
