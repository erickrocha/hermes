use axum::routing::{get, post, put};
use axum::{middleware, Router};
use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::user_endpoint::{add, change_password, get_by_id, list_all, update};

pub fn user_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/change-password", put(change_password))
        .route("/{id}", get(get_by_id))
        .route("/{id}", put(update))
        .route_layer(middleware::from_fn_with_state(
            state,
            authentication,
        ))
}
