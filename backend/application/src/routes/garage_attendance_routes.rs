use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::garage_attendance_endpoint::{active, call_list, checkout, get_by_uuid, mark_service, monitor, monitor_cards, monitor_matrix, open, queue};
use axum::routing::{get, post, put};
use axum::{Router, middleware};

/// `EPIC-GA-02` (`HRMS-958`/`959`, `AD-005`/`AD-006`): shaped like every other
/// tenant-owned resource.
pub fn garage_attendance_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(open))
        .route("/active", get(active))
        .route("/queue", get(queue))
        .route("/monitor", get(monitor))
        .route("/call-list", get(call_list))
        .route("/monitor/cards", get(monitor_cards))
        .route("/monitor/matrix", get(monitor_matrix))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route("/uuid/{uuid}/service/{service_uuid}", put(mark_service))
        .route("/uuid/{uuid}/checkout", post(checkout))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
