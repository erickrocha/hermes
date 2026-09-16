use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::business_plan_endpoint::{
    add, delete, get_by_id, get_by_uuid, list_all, update,
};
use axum::routing::{get, post};
use axum::{Router, middleware};

pub fn business_plan_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add).get(list_all))
        .route("/{id}", get(get_by_id).put(update).delete(delete))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
