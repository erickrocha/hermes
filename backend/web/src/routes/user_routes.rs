use axum::routing::{get, post, put};
use axum::{middleware, Router};
use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::user_endpoint::{add, change_password, get_by_uuid, list_all, reissue_invite, update};

/// HRMS-204/AD-010 (OBS-TP-05): an account is addressed by its UUID. The
/// `/{id}` forms were removed rather than kept alongside, because a public URL
/// carrying a sequential id discloses how many accounts exist — keeping both
/// left that disclosure in place. The numeric id remains the database key and
/// the value the authorization rules compare.
pub fn user_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/change-password", put(change_password))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route("/uuid/{uuid}", put(update))
        .route("/uuid/{uuid}/invite", post(reissue_invite))
        .route_layer(middleware::from_fn_with_state(
            state,
            authentication,
        ))
}
