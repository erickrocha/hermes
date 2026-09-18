use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::{NaiveDate, NaiveDateTime};
use entity::business_plan_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set, TryIntoModel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusinessPlan {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub name: String,
    pub price_in_cents: i64,
    pub available_users: i32,
    pub period_days: i32,
    pub payment_date: NaiveDate,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
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
            created_at: Some(model.created_at.naive_utc()),
            created_by: model.created_by,
            updated_at: Some(model.updated_at.naive_utc()),
            updated_by: model.updated_by,
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
            created_at: model.created_at.take().map(|value| value.naive_utc()),
            created_by: model.created_by.take().flatten(),
            updated_at: model.updated_at.take().map(|value| value.naive_utc()),
            updated_by: model.updated_by.take().flatten(),
        }
    }
}
