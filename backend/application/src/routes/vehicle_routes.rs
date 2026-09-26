use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::checklist_run_endpoint::possession;
use crate::endpoints::km_evolution_endpoint::{add as add_km_reading, history as km_history};
use crate::endpoints::vehicle_assignment_endpoint::{assign, current, end, history};
use crate::endpoints::vehicle_endpoint::{add, get_by_uuid, list_all, tracking, update};
use axum::routing::{get, post, put};
use axum::{Router, middleware};

/// EPIC-FO-01-S05 (HRMS-924, AD-005/AD-006): the fleet endpoints are shaped
/// like the tenant and user ones -- authenticated, addressed by UUID
/// (HRMS-204/AD-010), and never by sequential id.
pub fn vehicle_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", post(add))
        .route("/", get(list_all))
        .route("/uuid/{uuid}", get(get_by_uuid))
        .route("/uuid/{uuid}", put(update))
        .route("/uuid/{uuid}/tracking", get(tracking))
        .route("/uuid/{uuid}/assignment", post(assign))
        .route("/uuid/{uuid}/assignment", get(current))
        .route("/uuid/{uuid}/assignment/end", put(end))
        .route("/uuid/{uuid}/assignments", get(history))
        .route("/uuid/{uuid}/km-evolution", post(add_km_reading))
        .route("/uuid/{uuid}/km-evolutions", get(km_history))
        .route("/uuid/{uuid}/possession", get(possession))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
