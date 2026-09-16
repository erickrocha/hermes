use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use entity::business_plan_tier_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set, TryIntoModel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusinessPlanTier {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub business_plan_id: i64,
    pub up_to_users: i32,
    pub price_per_user_in_cents: i64,
}

pub struct BusinessPlanTierEntityMapper;

impl EntityMapper<BusinessPlanTier, Model, ActiveModel> for BusinessPlanTierEntityMapper {
    fn build_active_model(plan: BusinessPlanTier) -> ActiveModel {
        ActiveModel {
            id: plan.id.map_or(NotSet, Set),
            uuid: plan
                .uuid
                .map(|uuid| Set(string_to_bytes(&uuid)))
                .unwrap_or(NotSet),
            business_plan_id: Set(plan.business_plan_id),
            up_to_users: Set(plan.up_to_users),
            price_per_user_in_cents: Set(plan.price_per_user_in_cents),
        }
    }

    fn from_model(model: Model) -> BusinessPlanTier {
        BusinessPlanTier {
            id: Some(model.id),
            uuid: Some(bytes_para_string(model.uuid)),
            business_plan_id: model.business_plan_id,
            up_to_users: model.up_to_users,
            price_per_user_in_cents: model.price_per_user_in_cents,
        }
    }

    fn from_active_model(mut model: ActiveModel) -> BusinessPlanTier {
        if let Ok(model) = model.clone().try_into_model() {
            return Self::from_model(model);
        }
        BusinessPlanTier {
            id: model.id.take(),
            uuid: model.uuid.take().map(bytes_para_string),
            business_plan_id: model.business_plan_id.unwrap(),
            up_to_users: model.up_to_users.unwrap(),
            price_per_user_in_cents: model.price_per_user_in_cents.unwrap(),
        }
    }
}
