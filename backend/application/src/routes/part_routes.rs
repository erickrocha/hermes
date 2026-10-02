use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::part_endpoint::{add, add_entry, adjust, get_by_uuid, list_all};
use axum::routing::{get, post};
use axum::{Router, middleware};

/// `EPIC-SP-01-S01`/`EPIC-SP-02-S01` (`HRMS-800`/`801`, `AD-005`/`AD-006`):
/// shaped like every other tenant-owned resource.
pub fn part_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route("/uuid/{uuid}/entry", post(add_entry))
        .route("/uuid/{uuid}/adjustment", post(adjust))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
