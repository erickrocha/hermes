use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::purchase_order_endpoint::{add, cancel, get_by_uuid, list_all, mark_ordered, mark_purchased};
use axum::routing::{get, post, put};
use axum::{Router, middleware};

/// `EPIC-SP-03-S01` (`HRMS-802`, `AD-005`/`AD-006`): shaped like every other
/// tenant-owned resource.
pub fn purchase_order_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route("/uuid/{uuid}/mark-ordered", put(mark_ordered))
        .route("/uuid/{uuid}/mark-purchased", put(mark_purchased))
        .route("/uuid/{uuid}/cancel", put(cancel))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
