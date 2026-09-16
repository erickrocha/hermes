use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TenantPlanJson {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: i64,
    pub business_plan_id: i64,
    #[schema(value_type = String, format = Date)]
    pub payment_date: NaiveDate,
    pub active: bool,
}
