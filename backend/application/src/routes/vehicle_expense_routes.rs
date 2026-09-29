use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::vehicle_expense_endpoint::{add, get_by_uuid, list_all};
use axum::routing::{get, post};
use axum::{Router, middleware};

/// `EPIC-SP-04-S01` (`HRMS-803`, `AD-005`/`AD-006`): shaped like every other
/// tenant-owned resource.
pub fn vehicle_expense_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
