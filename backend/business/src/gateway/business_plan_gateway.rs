use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::domain::business_plan::{sort_tiers, BusinessPlan, BusinessPlanEntityMapper};
use entity::business_plan_entity::Model as BusinessPlanEntity;
use entity::{business_plan_entity, business_plan_tier_entity, tenant_plan_entity};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, EntityTrait, QueryFilter, QueryOrder,
    TransactionTrait, TryIntoModel,
};
use std::collections::HashMap;
use uuid::Uuid;

pub struct BusinessPlanGateway {
    db: DbConn,
}

impl BusinessPlanGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    pub async fn save(
        &self,
        plan: BusinessPlan,
    ) -> Result<(BusinessPlanEntity, Vec<business_plan_tier_entity::Model>), DbErr> {
        let transaction = self.db.begin().await?;

        let tiers = plan.tiers.clone();
        let saved: business_plan_entity::Model = BusinessPlanEntityMapper::build_active_model(plan)
            .save(&transaction)
            .await?
            .try_into_model()?;

        business_plan_tier_entity::Entity::delete_many()
            .filter(business_plan_tier_entity::Column::BusinessPlanId.eq(saved.id))
            .exec(&transaction)
            .await?;

        let mut saved_tiers = Vec::new();
        for tier in tiers {
            let inserted = business_plan_tier_entity::ActiveModel {
                id: sea_orm::NotSet,
                uuid: sea_orm::Set(Uuid::new_v4().as_bytes().to_vec()),
                business_plan_id: sea_orm::Set(saved.id),
                up_to_users: sea_orm::Set(tier.up_to_users),
                price_per_user_in_cents: sea_orm::Set(tier.price_per_user_in_cents),
            }
            .insert(&transaction)
            .await?;
            saved_tiers.push(inserted);
        }

        transaction.commit().await?;
        Ok((saved, saved_tiers))
    }

    pub async fn find_by_id(&self,id: i64) -> Result<Option<(business_plan_entity::Model, Vec<business_plan_tier_entity::Model>)>, DbErr> {
        let Some(plan) = business_plan_entity::Entity::find_by_id(id).one(&self.db).await? else {
            return Ok(None);
        };
        let mut tiers = business_plan_tier_entity::Entity::find()
            .filter(business_plan_tier_entity::Column::BusinessPlanId.eq(id))
            .all(&self.db)
            .await?;
        sort_tiers(&mut tiers);
        Ok(Some((plan, tiers)))
    }

    pub async fn find_by_uuid(&self,uuid: &str) -> Result<Option<(business_plan_entity::Model, Vec<business_plan_tier_entity::Model>)>, DbErr> {
        let Some(plan) = business_plan_entity::Entity::find()
            .filter(business_plan_entity::Column::Uuid.eq(string_to_bytes(uuid)))
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };
        let mut tiers = business_plan_tier_entity::Entity::find()
            .filter(business_plan_tier_entity::Column::BusinessPlanId.eq(plan.id))
            .all(&self.db)
            .await?;
        sort_tiers(&mut tiers);
        Ok(Some((plan, tiers)))
    }

    pub async fn find_all(
        &self,
    ) -> Result<Vec<(business_plan_entity::Model, Vec<business_plan_tier_entity::Model>)>, DbErr>
    {
        let plans = business_plan_entity::Entity::find()
            .order_by_desc(business_plan_entity::Column::Id)
            .all(&self.db)
            .await?;

        if plans.is_empty() {
            return Ok(Vec::new());
        }

        let plan_ids: Vec<i64> = plans.iter().map(|p| p.id).collect();
        let mut all_tiers = business_plan_tier_entity::Entity::find()
            .filter(business_plan_tier_entity::Column::BusinessPlanId.is_in(plan_ids))
            .all(&self.db)
            .await?;
        sort_tiers(&mut all_tiers);

        let mut tiers_by_plan: HashMap<i64, Vec<business_plan_tier_entity::Model>> = HashMap::new();
        for tier in all_tiers {
            tiers_by_plan
                .entry(tier.business_plan_id)
                .or_default()
                .push(tier);
        }

        let result = plans
            .into_iter()
            .map(|plan| {
                let tiers = tiers_by_plan.remove(&plan.id).unwrap_or_default();
                (plan, tiers)
            })
            .collect();

        Ok(result)
    }

    /// Remove o plano do catálogo. Recusa se algum tenant ainda o referencia;
    /// as faixas somem por `ON DELETE CASCADE`.
    pub async fn delete(&self, id: i64) -> Result<bool, DbErr> {
        if business_plan_entity::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .is_none()
        {
            return Ok(false);
        }

        let in_use = tenant_plan_entity::Entity::find()
            .filter(tenant_plan_entity::Column::BusinessPlanId.eq(id))
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
