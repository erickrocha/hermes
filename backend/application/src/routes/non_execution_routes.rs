use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::non_execution_endpoint::{evaluate, history, justify, list_all, record_manual};
use axum::routing::{get, post, put};
use axum::{Router, middleware};

/// `EPIC-MT-04` (`C-026`): planned maintenance that was not carried out.
pub fn non_execution_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(list_all))
        .route("/work-order/uuid/{uuid}", get(history))
        .route("/occurrence/uuid/{uuid}/justify", put(justify))
        .route("/manual", post(record_manual))
        .route("/evaluate", post(evaluate))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
