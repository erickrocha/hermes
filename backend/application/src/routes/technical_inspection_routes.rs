use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::technical_inspection_endpoint::{add, get_by_uuid};
use axum::routing::{get, post};
use axum::{Router, middleware};

/// `EPIC-MT-07-S09` (`HRMS-714`): shaped like every other tenant-owned resource.
pub fn technical_inspection_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
