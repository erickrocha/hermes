use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::daily_schedule_endpoint::{
    add as add_daily_schedule, history as daily_schedule_history, remove as remove_daily_schedule,
};
use crate::endpoints::schedule_exception_endpoint::{
    add as add_exception, history as exception_history, remove as remove_exception,
};
use crate::endpoints::transport_demand_allocation_endpoint::{
    add as add_allocation, history as allocation_history, update as update_allocation,
};
use crate::endpoints::transport_demand_endpoint::{add, get_by_uuid, list_all, update};
use axum::routing::{delete, get, post, put};
use axum::{Router, middleware};

/// `EPIC-SC-02-S01`…`S04` (`HRMS-603`…`606`, `AD-005`/`AD-006`): shaped like
/// every other tenant-owned resource.
pub fn transport_demand_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route("/uuid/{uuid}", put(update))
        .route("/uuid/{uuid}/allocation", post(add_allocation))
        .route("/uuid/{uuid}/allocations", get(allocation_history))
        .route(
            "/uuid/{uuid}/allocation/uuid/{allocation_uuid}",
            put(update_allocation),
        )
        .route("/uuid/{uuid}/daily-schedule", post(add_daily_schedule))
        .route("/uuid/{uuid}/daily-schedules", get(daily_schedule_history))
        .route(
            "/uuid/{uuid}/daily-schedule/uuid/{entry_uuid}",
            delete(remove_daily_schedule),
        )
        .route("/uuid/{uuid}/exception", post(add_exception))
        .route("/uuid/{uuid}/exceptions", get(exception_history))
        .route(
            "/uuid/{uuid}/exception/uuid/{exception_uuid}",
            delete(remove_exception),
        )
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
