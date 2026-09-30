use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::garage_attendance_endpoint::{active, checkout, get_by_uuid, mark_service, open, queue};
use axum::routing::{get, post, put};
use axum::{Router, middleware};

/// `EPIC-GA-02` (`HRMS-958`/`959`, `AD-005`/`AD-006`): shaped like every other
/// tenant-owned resource.
pub fn garage_attendance_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(open))
        .route("/active", get(active))
        .route("/queue", get(queue))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route("/uuid/{uuid}/service/{service_uuid}", put(mark_service))
        .route("/uuid/{uuid}/checkout", post(checkout))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
