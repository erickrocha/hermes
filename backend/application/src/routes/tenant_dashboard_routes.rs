use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::tenant_dashboard_endpoint::snapshot;
use axum::Router;
use axum::middleware;
use axum::routing::get;

/// Operational overview for a signed-in member of a tenant.
pub fn tenant_dashboard_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(snapshot))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
