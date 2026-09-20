use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::business_plan_endpoint::{add, delete, get_by_uuid, list_all, update};
use axum::routing::{get, post};
use axum::{Router, middleware};

/// HRMS-204/AD-010 (OBS-TP-05): plans are addressed by UUID only. The `/{id}`
/// forms were removed rather than kept alongside the UUID ones — leaving them
/// in place would have left the sequential id in public URLs, which is the
/// disclosure HRMS-204 exists to prevent.
pub fn business_plan_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add).get(list_all))
        .route("/uuid/{uuid}", get(get_by_uuid).put(update).delete(delete))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
