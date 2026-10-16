use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateBusinessPlanJson {
    pub name: String,
    pub price_in_cents: i64,
    pub price_per_extra_user_in_cents: i64,
    pub available_users: i32,
    pub period_days: i32,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBusinessPlanJson {
    pub name: String,
    pub price_in_cents: i64,
    pub price_per_extra_user_in_cents: i64,
    pub available_users: i32,
    pub period_days: i32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BusinessPlanJson {
    pub id: i64,
    pub uuid: String,
    pub name: String,
    pub price_in_cents: i64,
    pub price_per_extra_user_in_cents: i64,
    pub available_users: i32,
    pub period_days: i32,
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
    pub updated_at: NaiveDateTime,
    pub updated_by: Option<String>,
}
