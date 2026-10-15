use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::service_type_endpoint::{add, get_by_uuid, list_all};
use axum::routing::{get, post};
use axum::{Router, middleware};

/// `EPIC-MT-05-S01` (`HRMS-704`, `AD-005`/`AD-006`): shaped like every other
/// tenant-owned resource.
pub fn service_type_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
