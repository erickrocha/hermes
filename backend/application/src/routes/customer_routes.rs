use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::customer_day_off_endpoint::{
    add as add_day_off, history as day_off_history, remove as remove_day_off,
};
use crate::endpoints::customer_endpoint::{add, get_by_uuid, list_all, update};
use axum::routing::{delete, get, post, put};
use axum::{Router, middleware};

/// `EPIC-SC-01-S01`/`S02` (`HRMS-600`/`601`, `AD-005`/`AD-006`): shaped like
/// every other tenant-owned resource -- authenticated, addressed by UUID
/// (`HRMS-204`/`AD-010`), never by sequential id.
pub fn customer_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route("/uuid/{uuid}", put(update))
        .route("/uuid/{uuid}/day-off", post(add_day_off))
        .route("/uuid/{uuid}/day-offs", get(day_off_history))
        .route("/uuid/{uuid}/day-off/uuid/{day_off_uuid}", delete(remove_day_off))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
