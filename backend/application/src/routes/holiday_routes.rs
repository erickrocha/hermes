use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::holiday_endpoint::{add, list_all, remove};
use axum::routing::{delete, get, post};
use axum::{Router, middleware};

/// `EPIC-SC-01-S03` (`HRMS-602`, `AD-005`/`AD-006`): shaped like every other
/// tenant-owned resource.
pub fn holiday_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/uuid/{uuid}", delete(remove))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
