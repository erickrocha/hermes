use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::extra_trip_endpoint::{
    add, authorize_preparation_skip, get_by_uuid, import, import_preview, list_all, update,
};
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
        .route("/uuid/{uuid}/preparation-skip", post(authorize_preparation_skip))
        .route("/import", post(import))
        .route("/import/preview", post(import_preview))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
