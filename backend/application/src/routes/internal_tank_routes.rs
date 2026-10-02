use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::internal_tank_endpoint::{configure, get_current, stock};
use axum::routing::{get, put};
use axum::{Router, middleware};

/// `EPIC-FU-07-S01` (`HRMS-943`, `AD-005`/`AD-006`): a singleton per tenant,
/// so no `/uuid/{uuid}` sub-path -- `PUT` upserts, `GET` fetches the one row.
pub fn internal_tank_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", put(configure))
        .route("/", get(get_current))
        .route("/stock", get(stock))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
