use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::fuel_entry_endpoint::{add, average, delete, gauge, get_by_uuid, list_all, receipt, report, restore, sync, unify};
use axum::routing::{delete as http_delete, get, post};
use axum::{Router, middleware};

/// `EPIC-FU-01-S01` (`HRMS-942`, `AD-005`/`AD-006`): shaped like every other
/// tenant-owned resource.
pub fn fuel_entry_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/uuid/{uuid}", get(get_by_uuid).merge(http_delete(delete)))
        .route("/report", get(report))
        .route("/average", get(average))
        .route("/gauge", get(gauge))
        .route("/receipt", post(receipt))
        .route("/restore", post(restore))
        .route("/unify", post(unify))
        .route("/sync", post(sync))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
