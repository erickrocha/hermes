use crate::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use business::gateway::business_plan_gateway::BusinessPlanGateway;
use business::use_cases::business_plan_use_case::BusinessPlanUseCase;
use serde::Serialize;

const PUBLIC_PLAN_LIMIT: u64 = 50;

/// Plan catalogue shown on the public landing page. Only what a visitor
/// needs to compare prices; no id, uuid or audit fields.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicPlanJson {
    pub name: String,
    pub price_in_cents: i64,
    pub price_per_extra_user_in_cents: i64,
    pub available_users: i32,
    pub period_days: i32,
}

/// Like `/health`, deliberately outside `routes/*.rs` and `ApiDoc`: it is an
/// unauthenticated marketing surface, not part of the tenant-facing contract.
pub fn public_plans_route() -> Router<AppState> {
    Router::new().route(
        "/public/business-plan",
        get(|State(state): State<AppState>| async move {
            let use_case = BusinessPlanUseCase::new(BusinessPlanGateway::new(state.conn.as_ref().clone()));
            match use_case.find_page(0, PUBLIC_PLAN_LIMIT, None).await {
                Ok((plans, _)) => Ok(Json(into_public(plans))),
                Err(error) => {
                    log::error!("Public plans lookup failed: {}", error.message);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }),
    )
}

fn into_public(plans: Vec<business::domain::business_plan::BusinessPlan>) -> Vec<PublicPlanJson> {
    let mut plans: Vec<PublicPlanJson> = plans
        .into_iter()
        .map(|plan| PublicPlanJson {
            name: plan.name,
            price_in_cents: plan.price_in_cents,
            price_per_extra_user_in_cents: plan.price_per_extra_user_in_cents,
            available_users: plan.available_users,
            period_days: plan.period_days,
        })
        .collect();
    plans.sort_by_key(|plan| plan.price_in_cents);
    plans
}

#[cfg(test)]
mod tests {
    use super::into_public;
    use business::domain::business_plan::BusinessPlan;

    fn plan(name: &str, price: i64) -> BusinessPlan {
        BusinessPlan {
            id: Some(7),
            uuid: Some("secret-uuid".into()),
            name: name.into(),
            price_in_cents: price,
            price_per_extra_user_in_cents: 500,
            available_users: 10,
            period_days: 30,
            created_at: None,
            created_by: Some("admin".into()),
            updated_at: None,
            updated_by: None,
        }
    }

    #[test]
    fn exposes_only_pricing_fields_ordered_by_price() {
        let result = into_public(vec![plan("Pro", 9900), plan("Basic", 1900)]);
        assert_eq!(result[0].name, "Basic");
        assert_eq!(result[1].name, "Pro");
        let json = serde_json::to_string(&result[0]).unwrap();
        assert!(!json.contains("uuid") && !json.contains("createdBy") && !json.contains("\"id\""));
        assert!(json.contains("priceInCents") && json.contains("pricePerExtraUserInCents"));
    }
}
