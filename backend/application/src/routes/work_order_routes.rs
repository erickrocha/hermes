use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::work_order_endpoint::{add, add_item, cancel, conclude, get_by_uuid, list_all};
use axum::routing::{get, post, put};
use axum::{Router, middleware};

/// `EPIC-MT-01-S01`/`S02`/`S03` (`HRMS-700`/`701`/`702`, `AD-005`/`AD-006`):
/// shaped like every other tenant-owned resource.
pub fn work_order_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route("/uuid/{uuid}/item", post(add_item))
        .route("/uuid/{uuid}/conclude", put(conclude))
        .route("/uuid/{uuid}/cancel", put(cancel))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
