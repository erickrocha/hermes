use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::maintenance_notification_endpoint::{
    list_all, mark_all_read, mark_read, unread_count,
};
use axum::routing::{get, put};
use axum::{Router, middleware};

/// `EPIC-MT-06` (`C-026`): the maintenance staff's in-app inbox.
pub fn maintenance_notification_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(list_all))
        .route("/unread-count", get(unread_count))
        .route("/read-all", put(mark_all_read))
        .route("/uuid/{uuid}/read", put(mark_read))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
