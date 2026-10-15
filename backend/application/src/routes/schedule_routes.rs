use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::schedule_endpoint::effective;
use axum::routing::get;
use axum::{Router, middleware};

/// `EPIC-SC-04-S01` (`HRMS-610`): the effective schedule, computed on demand.
pub fn schedule_routes(state: AppState) -> Router<AppState> {
    Router::new().route("/effective", get(effective)).route_layer(middleware::from_fn_with_state(state, authentication))
}
