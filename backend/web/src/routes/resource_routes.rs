use crate::AppState;
use crate::authentication::authentication_middleware::authentication;
use crate::endpoints::city_endpoint::{
    get_by_id as get_city_by_id, get_by_province, import as import_cities, list_all as list_cities,
    save as save_city,
};
use crate::endpoints::province_endpoint::{
    get_by_id as get_province_by_id, import as import_provinces, list_all as list_provinces,
    list_countries, list_page as list_provinces_page, save as save_province,
};
use axum::routing::{get, post};
use axum::{Router, middleware};

pub fn resources_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/cities", get(list_cities))
        // PD-027: administração de dados de referência, só para o SysAdmin.
        .route("/city", post(save_city))
        .route("/city/import", post(import_cities))
        .route("/province", post(save_province))
        .route("/province/import", post(import_provinces))
        .route("/province/page", get(list_provinces_page))
        .route("/cities/by-province/{province_id}", get(get_by_province))
        .route("/city/{id}", get(get_city_by_id))
        .route("/province", get(list_provinces))
        .route("/country", get(list_countries))
        .route("/province/{id}", get(get_province_by_id))
        .route_layer(middleware::from_fn_with_state(state, authentication))
}
