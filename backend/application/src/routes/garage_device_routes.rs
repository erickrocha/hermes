use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::garage_device_endpoint::{
    issue_pairing_code, list, pair, reissue_pairing_code, revoke,
};
use axum::routing::{delete, get, post};
use axum::{Router, middleware};

/// HRMS-418: tenant-owner device management; `/pair` is explicitly public and
/// is authorized by its one-time code in the authentication middleware.
pub fn garage_device_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(list))
        .route("/pairing-code", post(issue_pairing_code))
        .route("/uuid/{uuid}/pairing-code", post(reissue_pairing_code))
        .route("/pair", post(pair))
        .route("/uuid/{uuid}", delete(revoke))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
