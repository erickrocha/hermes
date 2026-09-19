use axum::routing::{get, post, put};
use axum::{middleware, Router};
use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::tenant_endpoint::{
    add, add_plan, add_plan_by_uuid, get_active_plan, get_active_plan_by_uuid, get_by_id,
    get_by_uuid, list_all, update, update_by_uuid,
};

pub fn tenant_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/{id}", get(get_by_id))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route("/{id}", put(update))
        .route("/{id}/plan", post(add_plan).get(get_active_plan))
        // HRMS-204/AD-010 (DEF-TP-04): the same operations addressed by the
        // tenant's public identifier, so no URL has to carry a sequential id.
        .route("/uuid/{uuid}", put(update_by_uuid))
        .route("/uuid/{uuid}/plan", post(add_plan_by_uuid).get(get_active_plan_by_uuid))
        .route_layer(middleware::from_fn_with_state(state,authentication))
}
