use axum::routing::{get, post, put};
use axum::{middleware, Router};
use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::vehicle_endpoint::{add, get_by_uuid, list_all, update};

/// EPIC-FO-01-S05 (HRMS-924, AD-005/AD-006): the fleet endpoints are shaped
/// like the tenant and user ones -- authenticated, addressed by UUID
/// (HRMS-204/AD-010), and never by sequential id.
pub fn vehicle_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route("/uuid/{uuid}", put(update))
        .route_layer(middleware::from_fn_with_state(
            state,
            authentication,
        ))
}
