use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::Gateway;
use crate::domain::tenant_plan::{TenantPlan, TenantPlanEntityMapper};
use entity::prelude::TenantPlanEntity as TenantPlanQuery;
use entity::tenant_plan_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter, Set};

pub struct TenantPlanGateway {
    db: DbConn,
}

impl TenantPlanGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    pub async fn deactivate_all_active_plans_for_tenant(&self, tenant_id: i64) -> Result<(), DbErr> {
        let active_plans = TenantPlanQuery::find()
            .filter(tenant_plan_entity::Column::TenantId.eq(tenant_id))
            .filter(tenant_plan_entity::Column::Active.eq(true))
            .all(&self.db)
            .await?;

        for plan in active_plans {
            let mut active_model: tenant_plan_entity::ActiveModel = plan.into();
            active_model.active = Set(false);
            active_model.save(&self.db).await?;
        }

        Ok(())
    }

    pub async fn find_active_by_tenant_id(&self, tenant_id: i64) -> Result<Option<tenant_plan_entity::Model>, DbErr> {
        TenantPlanQuery::find()
            .filter(tenant_plan_entity::Column::TenantId.eq(tenant_id))
            .filter(tenant_plan_entity::Column::Active.eq(true))
            .one(&self.db)
            .await
    }
}

#[async_trait]
impl Gateway<TenantPlan, tenant_plan_entity::Model, tenant_plan_entity::ActiveModel> for TenantPlanGateway {
    async fn persist(&self, entity: TenantPlan) -> Result<tenant_plan_entity::ActiveModel, DbErr> {
        let active_model = TenantPlanEntityMapper::build_active_model(entity);
        active_model.save(&self.db).await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        TenantPlanQuery::delete_by_id(id)
            .exec(&self.db)
            .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<tenant_plan_entity::Model>, DbErr> {
        TenantPlanQuery::find()
            .filter(tenant_plan_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<tenant_plan_entity::Model>, DbErr> {
        TenantPlanQuery::find()
            .filter(tenant_plan_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<tenant_plan_entity::Model>, DbErr> {
        TenantPlanQuery::find().all(&self.db).await
    }
}
