use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::cost_report_endpoint::{
    suppliers, supplier_closing, supplier_closing_export, vehicle_cost, vehicle_cost_export, work_order_cost,
};
use axum::routing::get;
use axum::{Router, middleware};

/// `C-030` `EPIC-SP-07`/`SP-08` (`HRMS-804…810`): read-only reports over built data.
pub fn cost_report_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/vehicle/uuid/{uuid}", get(vehicle_cost))
        .route("/vehicle/uuid/{uuid}/export", get(vehicle_cost_export))
        .route("/work-order/uuid/{uuid}", get(work_order_cost))
        .route("/suppliers", get(suppliers))
        .route("/supplier-closing", get(supplier_closing))
        .route("/supplier-closing/export", get(supplier_closing_export))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
