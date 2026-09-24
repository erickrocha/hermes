use axum::{middleware, Router};
use axum::routing::post;
use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::auth_endpoint::{accept_invite, refresh_token, sign_in};

/// Build authentication routes (signup and login, no auth required, others auth is required)
pub fn auth_routes(state: AppState) -> Router<AppState> {
	Router::new()

		.route("/login", post(sign_in).route_layer(middleware::from_fn_with_state(
			state.clone(),
			authentication,
		)))
		.route("/refresh", post(refresh_token).route_layer(middleware::from_fn_with_state(
			state.clone(),
			authentication,
		)))
		.route("/accept-invite", post(accept_invite).route_layer(middleware::from_fn_with_state(
			state.clone(),
			authentication,
		)))

}