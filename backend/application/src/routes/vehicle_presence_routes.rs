use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::vehicle_presence_endpoint::{record, snapshot};
use axum::routing::{get, post};
use axum::{Router, middleware};

/// `EPIC-GA-03-S01` (`HRMS-960`): the presence domain's published stamps.
pub fn vehicle_presence_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(record))
        .route("/", get(snapshot))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
