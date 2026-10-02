use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::inspection_model_endpoint::{add, get_by_uuid, list_all};
use axum::routing::{get, post};
use axum::{Router, middleware};

/// `EPIC-MT-07-S10` (`HRMS-715`): shaped like every other tenant-owned resource.
pub fn inspection_model_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
