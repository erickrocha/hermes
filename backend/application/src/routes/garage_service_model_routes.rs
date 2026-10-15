use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::garage_service_model_endpoint::{add, get_by_uuid, list_all, update};
use axum::routing::{get, post};
use axum::{Router, middleware};

/// `EPIC-GA-01-S01` (`HRMS-956`, `AD-005`/`AD-006`): shaped like every other
/// tenant-owned resource.
pub fn garage_service_model_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/uuid/{uuid}", get(get_by_uuid).put(update))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
