use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::billing_endpoint::{
    charges, confirm_charge, get_settings, set_grace_days, set_suspension,
};
use axum::routing::{get, post, put};
use axum::{Router, middleware};

pub fn billing_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/charges", get(charges))
        .route("/charges/uuid/{uuid}/confirm", post(confirm_charge))
        .route("/tenant/uuid/{uuid}/settings", get(get_settings).put(set_grace_days))
        .route("/tenant/uuid/{uuid}/suspension", put(set_suspension))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}