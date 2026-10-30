use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::cross_tenant_action_endpoint::list_all;
use axum::routing::get;
use axum::{Router, middleware};

/// `EPIC-AC-08-S06` (`HRMS-877`): the trail of what a tenant did inside its descendants.
pub fn cross_tenant_action_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(list_all))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
