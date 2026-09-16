use axum::routing::{get, post, put};
use axum::{middleware, Router};
use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::tenant_endpoint::{add, get_by_id, get_by_uuid, list_all, update, add_plan, get_active_plan};

pub fn tenant_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/{id}", get(get_by_id))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route("/{id}", put(update))
        .route("/{id}/plan", post(add_plan).get(get_active_plan))
        .route_layer(middleware::from_fn_with_state(state,authentication))
}
