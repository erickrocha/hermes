use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, NotFoundErrorJson, UnauthorizedErrorJson, ForbiddenErrorJson,
    InternalServerErrorJson,
};
use crate::endpoints::json::city_json::CityJson;
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::infrastructure::mapper::{Mapper, CityMapper};
use crate::AppState;
use axum::extract::{Path, Query, State};
use axum::extract::Extension;
use axum::Json;
use business::gateway::city_gateway::CityGateway;
use business::use_cases::city_use_case::CityUseCase;
use business::domain::authorization::can_manage_reference_data;
use business::domain::city::City;
use business::domain::user::User;
use business::use_cases::reference_import::ImportOutcome;
use crate::endpoints::json::reference_json::ImportResultJson;

#[utoipa::path(
    get,
    tag = "City",
    path = "/cities",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of cities (PD-028). The largest table in the system, and the reason the rule is project-wide. The address dropdowns do NOT use this route -- they use /cities/by-province/{province_id}, which stays unpaginated on purpose: a truncated option list is defect D-9 again.", body = PageJson<CityJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_all(
    state: State<AppState>,
    Query(page_query): Query<PageQuery>,
    Extension(locale): Extension<Locale>,
) -> HttpResponse<Json<PageJson<CityJson>>> {
    let use_case = CityUseCase::new(CityGateway::new(state.conn.as_ref().clone()));
    let (page, page_size) = (page_query.page(), page_query.page_size());
    let search = page_query.search();
    let (list, total) = use_case
        .find_page(page, page_size, search.as_deref())
        .await
        .map_err(|_| ExceptionResponse::InternalServerError(locale, ErrorKey::ReferenceDataUnavailable))?;
    Ok(Json(PageJson::new(CityMapper::json_vec(list), page, page_size, total)))
}

#[utoipa::path(
    get,
    tag = "City",
    path = "/cities/by-province/{province_id}",
    params(
        ("province_id" = i32, Path, description = "Province ID")
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
    Extension(locale): Extension<Locale>,
    Path(province_id): Path<i32>,
) -> HttpResponse<Json<Vec<CityJson>>> {
    let use_case = CityUseCase::new(CityGateway::new(state.conn.as_ref().clone()));
    let list = use_case
        .find_by_province_id(province_id)
        .await
        .map_err(|_| ExceptionResponse::InternalServerError(locale, ErrorKey::ReferenceDataUnavailable))?;
    Ok(Json(CityMapper::json_vec(list)))
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

fn authorize(user: &User, locale: &Locale) -> Result<(), ExceptionResponse> {
    if !can_manage_reference_data(user) {
        return Err(ExceptionResponse::Forbidden(locale.clone(), ErrorKey::BusinessPlanForbidden));
    }
    Ok(())
}

fn domain(json: CityJson) -> City {
    City { id: json.id, uuid: json.uuid, province_id: json.province_id, name: json.name }
}

#[utoipa::path(
    post,
    tag = "City",
    path = "/city",
    request_body = CityJson,
    responses(
        (status = 200, description = "City saved", body = CityJson),
        (status = 400, body = BadRequestErrorJson),
        (status = 401, body = UnauthorizedErrorJson),
        (status = 403, body = ForbiddenErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn save(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(user): Extension<User>,
    Json(payload): Json<CityJson>,
) -> HttpResponse<Json<CityJson>> {
    authorize(&user, &locale)?;
    let use_case = CityUseCase::new(CityGateway::new(state.conn.as_ref().clone()));
    use_case
        .save(domain(payload))
        .await
        .map(|saved| Json(CityMapper::json(saved)))
        .map_err(|error| ExceptionResponse::BadRequestMessage(error.message))
}

/// PD-027: recebe as linhas que o operador já revisou na grade. Tudo ou nada.
#[utoipa::path(
    post,
    tag = "City",
    path = "/city/import",
    request_body = Vec<CityJson>,
    responses(
        (status = 200, description = "Import applied", body = ImportResultJson),
        (status = 400, description = "Nothing was written; the message names each rejected row", body = BadRequestErrorJson),
        (status = 401, body = UnauthorizedErrorJson),
        (status = 403, body = ForbiddenErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn import(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(user): Extension<User>,
    Json(payload): Json<Vec<CityJson>>,
) -> HttpResponse<Json<ImportResultJson>> {
    authorize(&user, &locale)?;
    let use_case = CityUseCase::new(CityGateway::new(state.conn.as_ref().clone()));
    use_case
        .import(payload.into_iter().map(domain).collect())
        .await
        .map(|result: ImportOutcome| Json(ImportResultJson { created: result.created, updated: result.updated }))
        .map_err(|error| ExceptionResponse::BadRequestMessage(error.message))
}
