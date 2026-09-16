use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::city_endpoint::{
    get_by_id as get_city_by_id, get_by_province, list_all as list_cities,
};
use crate::endpoints::province_endpoint::{
    get_by_id as get_province_by_id, list_all as list_provinces,
};
use axum::routing::get;
use axum::{Router, middleware};

pub fn resources_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/cities", get(list_cities))
        .route("/cities/by-province/{province_id}", get(get_by_province))
        .route("/city/by-province/{province_id}", get(get_by_province))
        .route("/city/{id}", get(get_city_by_id))
        .route("/province", get(list_provinces))
        .route("/province/{id}", get(get_province_by_id))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
