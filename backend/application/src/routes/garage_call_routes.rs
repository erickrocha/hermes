use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::garage_call_endpoint::{call, cancel, get_active};
use axum::routing::{get, post};
use axum::{Router, middleware};

/// `EPIC-GA-05-S02` (`HRMS-963`): the manager's manual call to base.
pub fn garage_call_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(call))
        .route("/", get(get_active))
        .route("/cancel", post(cancel))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
