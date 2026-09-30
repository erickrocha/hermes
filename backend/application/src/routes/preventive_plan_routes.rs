use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::preventive_plan_endpoint::{add, extend, generate_work_order, get_by_uuid, list_all, update};
use axum::routing::{get, post};
use axum::{Router, middleware};

/// `EPIC-MT-07-S01` (`HRMS-706`, `AD-005`/`AD-006`): shaped like every other
/// tenant-owned resource.
pub fn preventive_plan_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/uuid/{uuid}", get(get_by_uuid).put(update))
        .route("/uuid/{uuid}/work-order", post(generate_work_order))
        .route("/uuid/{uuid}/extension", post(extend))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
