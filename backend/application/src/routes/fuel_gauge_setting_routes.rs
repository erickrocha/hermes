use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::fuel_gauge_setting_endpoint::{configure, get_current};
use axum::routing::{get, put};
use axum::{Router, middleware};

/// `EPIC-FU-06-S03` (`HRMS-953`): a singleton per tenant, like `/internal-tank`.
pub fn fuel_gauge_setting_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", put(configure))
        .route("/", get(get_current))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
