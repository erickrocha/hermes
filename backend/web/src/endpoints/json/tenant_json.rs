use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TenantJson {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub business_name: Option<String>,
    pub company_name: Option<String>,
    pub tax_id: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub website: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub locality: Option<String>,
    pub administrative_area: Option<String>,
    pub postal_code: Option<String>,
    pub country_code: Option<String>,
    pub province: Option<String>,
    pub city: Option<String>,
    pub zipcode: Option<String>,
    /// Days a customer keeps app access after a charge falls due. Omitting it
    /// on an update keeps the clinic's current value.
    /// The tenant's current plan. Read-only here — set only through
    /// `POST /tenant/{id}/plan` (PD-021); ignored on create/update.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub business_plan_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<chrono::NaiveDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<chrono::NaiveDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_by: Option<String>,
}

/// Request body for `POST /tenant/{id}/plan` (HRMS-224). Replaces the old
/// `TenantPlanJson`, which carried a `paymentDate`/`active` pair that no
/// longer exists — PD-021 keeps no plan history.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetTenantPlanJson {
    pub business_plan_id: i64,
}
