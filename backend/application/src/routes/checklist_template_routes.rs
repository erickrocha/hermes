use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::checklist_template_endpoint::{add, get_by_uuid, list_all};
use axum::routing::{get, post};
use axum::{Router, middleware};

/// `EPIC-CK-02-S01` (`HRMS-651`, `AD-005`/`AD-006`): shaped like every other
/// tenant-owned resource.
pub fn checklist_template_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
