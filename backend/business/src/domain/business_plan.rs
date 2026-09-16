use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::{NaiveDate, NaiveDateTime};
use entity::business_plan_entity::{ActiveModel, Model};
use entity::business_plan_tier_entity;
use sea_orm::{NotSet, Set, TryIntoModel};
use crate::domain::business_plan_tier::{BusinessPlanTier, BusinessPlanTierEntityMapper};

/// Ordena as faixas para o caminhamento de preço: a faixa sem teto
/// (`up_to_users = 0`) tem de ser a última, as demais em ordem crescente.
pub fn sort_tiers(tiers: &mut [business_plan_tier_entity::Model]) {
    tiers.sort_by_key(|tier| {
        if tier.up_to_users == 0 {
            i32::MAX
        } else {
            tier.up_to_users
        }
    });
}



#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusinessPlan {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub name: String,
    pub price_in_cents: i64,
    pub available_users: i32,
    pub period_days: i32,
    pub payment_date: NaiveDate,
    pub daily_ai_quota: i32,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
    pub tiers: Vec<BusinessPlanTier>,
}

pub struct BusinessPlanEntityMapper;

impl EntityMapper<BusinessPlan, Model, ActiveModel> for BusinessPlanEntityMapper {
    fn build_active_model(plan: BusinessPlan) -> ActiveModel {
        ActiveModel {
            id: plan.id.map_or(NotSet, Set),
            uuid: plan
                .uuid
                .map(|uuid| Set(string_to_bytes(&uuid)))
                .unwrap_or(NotSet),
            name: Set(plan.name),
            price_in_cents: Set(plan.price_in_cents),
            available_users: Set(plan.available_users),
            period_days: Set(plan.period_days),
            payment_date: Set(plan.payment_date),
            daily_ai_quota: Set(if plan.daily_ai_quota <= 0 {
                4
            } else {
                plan.daily_ai_quota
            }),
            created_at: NotSet,
            created_by: plan
                .created_by
                .map(|value| Set(Some(value)))
                .unwrap_or(NotSet),
            updated_at: NotSet,
            updated_by: plan
                .updated_by
                .map(|value| Set(Some(value)))
                .unwrap_or(NotSet),
        }
    }

    fn from_model(model: Model) -> BusinessPlan {
        BusinessPlan {
            id: Some(model.id),
            uuid: Some(bytes_para_string(model.uuid)),
            name: model.name,
            price_in_cents: model.price_in_cents,
            available_users: model.available_users,
            period_days: model.period_days,
            payment_date: model.payment_date,
            daily_ai_quota: model.daily_ai_quota,
            created_at: Some(model.created_at.naive_utc()),
            created_by: model.created_by,
            updated_at: Some(model.updated_at.naive_utc()),
            updated_by: model.updated_by,
            tiers: Vec::new(),
        }
    }

    fn from_active_model(mut model: ActiveModel) -> BusinessPlan {
        if let Ok(model) = model.clone().try_into_model() {
            return Self::from_model(model);
        }
        BusinessPlan {
            id: model.id.take(),
            uuid: model.uuid.take().map(bytes_para_string),
            name: model.name.take().unwrap_or_default(),
            price_in_cents: model.price_in_cents.take().unwrap_or_default(),
            available_users: model.available_users.take().unwrap_or_default(),
            period_days: model.period_days.take().unwrap_or_default(),
            payment_date: model.payment_date.take().unwrap_or_default(),
            daily_ai_quota: model.daily_ai_quota.take().unwrap_or(4),
            created_at: model.created_at.take().map(|value| value.naive_utc()),
            created_by: model.created_by.take().flatten(),
            updated_at: model.updated_at.take().map(|value| value.naive_utc()),
            updated_by: model.updated_by.take().flatten(),
            tiers: Vec::new(),
        }
    }
}

impl BusinessPlanEntityMapper {
    /// Constrói o domínio já com as faixas anexadas (ordenadas por `sort_tiers`).
    pub fn from_model_with_tiers(
        model: Model,
        tiers: Vec<business_plan_tier_entity::Model>,
    ) -> BusinessPlan {
        let mut plan = <Self as EntityMapper<BusinessPlan, Model, ActiveModel>>::from_model(model);
        plan.tiers = tiers
            .into_iter()
            .map(BusinessPlanTierEntityMapper::from_model)
            .collect();
        plan
    }
}

#[cfg(test)]
mod tests {
    use super::sort_tiers;
    use entity::business_plan_tier_entity;
    use crate::commons::functions::string_to_bytes;

    fn tier(id: i64, up_to_users: i32) -> business_plan_tier_entity::Model {
        business_plan_tier_entity::Model {
            id,
            uuid: string_to_bytes("test_uuid"),
            business_plan_id: 1,
            up_to_users,
            price_per_user_in_cents: 100,
        }
    }

    #[test]
    fn open_ended_tier_goes_last() {
        // Ordem em que o MariaDB devolve com ORDER BY up_to_users ASC: NULL primeiro.
        let mut tiers = vec![tier(3, 0), tier(1, 50), tier(2, 150)];
        sort_tiers(&mut tiers);
        assert_eq!(
            tiers.iter().map(|t| t.up_to_users).collect::<Vec<_>>(),
            vec![50, 150, 0]
        );
    }
}
