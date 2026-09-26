use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::tenant_endpoint::{
    add, add_plan_by_uuid, get_active_plan_by_uuid, get_by_uuid, list_all, update_by_uuid,
};
use axum::routing::{get, post, put};
use axum::{Router, middleware};

pub fn tenant_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        // HRMS-204/AD-010 (DEF-TP-04, OBS-TP-05): tenants are addressed only by
        // their public UUID. The by-id twins of these operations were removed so
        // that no public URL carries the sequential internal identifier.
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route("/uuid/{uuid}", put(update_by_uuid))
        .route(
            "/uuid/{uuid}/plan",
            post(add_plan_by_uuid).get(get_active_plan_by_uuid),
        )
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
