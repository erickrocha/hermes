use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::fuel_entry_endpoint::{add, get_by_uuid, list_all, report, sync};
use axum::routing::{get, post};
use axum::{Router, middleware};

/// `EPIC-FU-01-S01` (`HRMS-942`, `AD-005`/`AD-006`): shaped like every other
/// tenant-owned resource.
pub fn fuel_entry_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route("/report", get(report))
        .route("/sync", post(sync))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
