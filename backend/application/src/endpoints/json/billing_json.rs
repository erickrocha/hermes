use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BillingChargeJson {
    pub uuid: String,
    pub tenant_uuid: String,
    pub tenant_name: String,
    pub plan_name: String,
    pub currency: String,
    pub period_start: NaiveDateTime,
    pub period_end: NaiveDateTime,
    pub price_in_cents: i64,
    pub price_per_extra_user_in_cents: i64,
    pub included_users: i32,
    pub active_users: i32,
    pub extra_users: i32,
    pub grace_days: i32,
    pub total_in_cents: i64,
    pub status: String,
    pub is_overdue: bool,
    pub paid_at: Option<NaiveDateTime>,
    pub paid_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BillingSettingsJson {
    pub tenant_uuid: String,
    pub grace_days: Option<i32>,
    pub cycle_started_at: Option<NaiveDateTime>,
    pub pending_plan: bool,
    pub suspended_at: Option<NaiveDateTime>,
    pub suspended_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigureBillingSettingsJson {
    pub grace_days: i32,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BillingSuspensionJson {
    pub suspended: bool,
}