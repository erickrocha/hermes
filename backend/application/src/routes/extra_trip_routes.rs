use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::extra_trip_endpoint::{add, get_by_uuid, import, list_all, update};
use axum::routing::{get, post, put};
use axum::{Router, middleware};

/// `EPIC-SC-03-S01`/`S02` (`HRMS-607`/`608`, `AD-005`/`AD-006`): shaped like
/// every other tenant-owned resource.
pub fn extra_trip_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route("/uuid/{uuid}", put(update))
        .route("/import", post(import))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
