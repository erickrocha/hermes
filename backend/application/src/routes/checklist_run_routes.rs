use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::checklist_run_endpoint::{add, get_by_uuid};
use axum::routing::{get, post};
use axum::{Router, middleware};

/// `EPIC-CK-03-S01` (`HRMS-652`, `AD-005`/`AD-006`): shaped like every other
/// tenant-owned resource. The vehicle's current holder is read at
/// `/vehicle/uuid/{uuid}/possession` instead, alongside its other
/// sub-resources.
pub fn checklist_run_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
