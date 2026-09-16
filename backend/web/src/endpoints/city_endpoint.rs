use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    NotFoundErrorJson, UnauthorizedErrorJson, ForbiddenErrorJson, InternalServerErrorJson,
};
use crate::endpoints::json::city_json::CityJson;
use crate::infrastructure::mapper::{Mapper, CityMapper};
use crate::AppState;
use axum::extract::{Path, State};
use axum::extract::Extension;
use axum::Json;
use business::gateway::city_gateway::CityGateway;
use business::use_cases::city_use_case::CityUseCase;

#[utoipa::path(
    get,
    tag = "City",
    path = "/city",
    responses(
        (status = 200, description = "List of all cities", body = Vec<CityJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_all(
    state: State<AppState>,
) -> HttpResponse<Json<Vec<CityJson>>> {
    let use_case = CityUseCase::new(CityGateway::new(state.conn.as_ref().clone()));
    match use_case.find_all().await {
        Ok(list) => Ok(Json(CityMapper::json_vec(list))),
        Err(_) => Ok(Json(Vec::new())),
    }
}

#[utoipa::path(
    get,
    tag = "City",
    path = "/city/by-province/{province_id}",
    params(
        ("province_id" = i64, Path, description = "Province ID")
    ),
    responses(
        (status = 200, description = "List of cities for the specified province", body = Vec<CityJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_by_province(
    state: State<AppState>,
    Path(province_id): Path<i32>,
) -> HttpResponse<Json<Vec<CityJson>>> {
    let use_case = CityUseCase::new(CityGateway::new(state.conn.as_ref().clone()));
    match use_case.find_by_province_id(province_id).await {
        Ok(list) => Ok(Json(CityMapper::json_vec(list))),
        Err(_) => Ok(Json(Vec::new())),
    }
}

#[utoipa::path(
    get,
    tag = "City",
    path = "/city/{id}",
    params(
        ("id" = i64, Path, description = "City ID")
    ),
    responses(
        (status = 200, description = "City found", body = CityJson),
        (status = 404, description = "City not found", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_by_id(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Path(id): Path<i64>,
) -> HttpResponse<Json<CityJson>> {
    let use_case = CityUseCase::new(CityGateway::new(state.conn.as_ref().clone()));
    match use_case.find_by_id(id).await {
        Ok(res) => Ok(Json(CityMapper::json(res))),
        Err(_) => Err(ExceptionResponse::NotFound(locale, ErrorKey::RequiredParameterMissing)),
    }
}
