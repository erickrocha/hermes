use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::preventive_plan_endpoint::{
    add, add_alert, discharge_alert, extend, generate_work_order, get_by_uuid, list_all, pending_alerts, update,
};
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
        .route("/uuid/{uuid}/alert", post(add_alert).get(pending_alerts))
        .route("/alert/{uuid}/discharge", post(discharge_alert))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
