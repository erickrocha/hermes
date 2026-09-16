use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

fn default_daily_ai_quota() -> i32 {
    4
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BusinessPlanTierJson {
    pub id: Option<i64>,
    /// Teto de usuários da faixa; `0` marca a faixa sem teto (a última).
    pub up_to_users: i32,
    pub price_per_user_in_cents: i64,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateBusinessPlanJson {
    pub name: String,
    pub price_in_cents: i64,
    pub available_users: i32,
    pub period_days: i32,
    pub payment_date: NaiveDate,
    #[serde(default = "default_daily_ai_quota")]
    pub daily_ai_quota: i32,
    pub tiers: Option<Vec<BusinessPlanTierJson>>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBusinessPlanJson {
    pub name: String,
    pub price_in_cents: i64,
    pub available_users: i32,
    pub period_days: i32,
    pub payment_date: NaiveDate,
    #[serde(default = "default_daily_ai_quota")]
    pub daily_ai_quota: i32,
    pub tiers: Option<Vec<BusinessPlanTierJson>>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BusinessPlanJson {
    pub id: i64,
    pub uuid: String,
    pub name: String,
    pub price_in_cents: i64,
    pub available_users: i32,
    pub period_days: i32,
    pub payment_date: NaiveDate,
    pub daily_ai_quota: i32,
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
    pub updated_at: NaiveDateTime,
    pub updated_by: Option<String>,
    pub tiers: Vec<BusinessPlanTierJson>,
}
