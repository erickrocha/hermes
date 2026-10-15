use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::access_profile_endpoint::{add, assign, catalogue, get_by_uuid, list_all, mine, update, user_permissions};
use axum::routing::{get, post, put};
use axum::{Router, middleware};

/// `C-032` (`HRMS-850…853`, `D-27`): the tenant's access profiles, the permission catalogue, and what
/// the caller may do now.
pub fn access_profile_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/catalogue", get(catalogue))
        .route("/mine", get(mine))
        .route("/uuid/{uuid}", get(get_by_uuid).put(update))
        .route("/user/uuid/{user_uuid}", put(assign).get(user_permissions))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
