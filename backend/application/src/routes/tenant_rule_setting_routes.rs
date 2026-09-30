use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::tenant_rule_setting_endpoint::{configure, get_current};
use axum::routing::{get, put};
use axum::{Router, middleware};

/// `HRMS-954`: a singleton per tenant, like `/internal-tank`.
pub fn tenant_rule_setting_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", put(configure))
        .route("/", get(get_current))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
