use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale, translate_reference_data_error};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::json::province_json::ProvinceJson;
use crate::endpoints::json::reference_json::ImportResultJson;
use crate::infrastructure::mapper::{Mapper, ProvinceMapper};
use axum::Json;
use axum::extract::Extension;
use axum::extract::{Path, Query, State};
use business::domain::authorization::can_manage_reference_data;
use business::domain::province::{Province, normalize_country_code};
use business::domain::user::User;
use business::gateway::province_gateway::ProvinceGateway;
use business::use_cases::province_use_case::ProvinceUseCase;
use business::use_cases::reference_import::{ImportOutcome, ReferenceDataError};

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ProvinceQueryParams {
    /// ISO 3166-1 alpha-2 country code.
    pub country_code: String,
}

#[utoipa::path(
    get,
    tag = "Province",
    path = "/province",
    params(ProvinceQueryParams),
    responses(
        (status = 200, description = "List of provinces. **Roles:** SysAdmin, TenantOwner, TenantUser, Driver, Mechanic (any authenticated role).", body = Vec<ProvinceJson>),
        (status = 400, description = "Missing or invalid country code", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 404, description = "No reference data seeded for this country", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_all(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Query(params): Query<ProvinceQueryParams>,
) -> HttpResponse<Json<Vec<ProvinceJson>>> {
    let country_code = normalize_country_code(&params.country_code).ok_or(
        ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue),
    )?;

    let use_case = ProvinceUseCase::new(ProvinceGateway::new(state.conn.as_ref().clone()));
    let list = use_case
        .find_by_country_code(country_code)
        .await
        .map_err(|_| {
            ExceptionResponse::InternalServerError(
                locale.clone(),
                ErrorKey::ReferenceDataUnavailable,
            )
        })?;

    // EPIC-RD-01-S04/HRMS-306 (D-9): a country with no seeded provinces is a
    // country we don't support yet, not a query that happens to be empty --
    // the caller needs to be able to tell those apart rather than see a
    // silent empty dropdown either way.
    if list.is_empty() {
        return Err(ExceptionResponse::NotFound(
            locale,
            ErrorKey::CountryNotSupported,
        ));
    }

    Ok(Json(ProvinceMapper::json_vec(list)))
}

#[utoipa::path(
    get,
    tag = "Province",
    path = "/province/{id}",
    params(
        ("id" = i32, Path, description = "Province ID")
    ),
    responses(
        (status = 200, description = "Province found. **Roles:** SysAdmin, TenantOwner, TenantUser, Driver, Mechanic (any authenticated role).", body = ProvinceJson),
        (status = 404, description = "Province not found", body = NotFoundErrorJson),
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
) -> HttpResponse<Json<ProvinceJson>> {
    let use_case = ProvinceUseCase::new(ProvinceGateway::new(state.conn.as_ref().clone()));
    match use_case.find_by_id(id).await {
        Ok(res) => Ok(Json(ProvinceMapper::json(res))),
        // DEF-RD-06: "required parameter missing" described the caller's
        // request, which was fine; what was missing was the record.
        Err(error) if error.is_not_found() => Err(ExceptionResponse::NotFound(
            locale,
            ErrorKey::ProvinceNotFound,
        )),
        Err(_) => Err(ExceptionResponse::InternalServerError(
            locale,
            ErrorKey::ReferenceDataUnavailable,
        )),
    }
}

/// DEF-RD-08 (PD-022/PD-027): the countries a tenant can be placed in, which
/// is exactly the set that has provinces. Open to any signed-in caller: the
/// tenant editor needs it, and a list of ISO country codes is not sensitive.
/// Deriving it means importing a country's provinces is all it takes to make
/// that country selectable, with no code change.
#[utoipa::path(
    get,
    tag = "Province",
    path = "/country",
    responses(
        (status = 200, description = "Country codes with reference data. **Roles:** SysAdmin, TenantOwner, TenantUser, Driver, Mechanic (any authenticated role).", body = Vec<String>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_countries(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
) -> HttpResponse<Json<Vec<String>>> {
    let use_case = ProvinceUseCase::new(ProvinceGateway::new(state.conn.as_ref().clone()));
    let codes = use_case.find_countries().await.map_err(|_| {
        ExceptionResponse::InternalServerError(locale, ErrorKey::ReferenceDataUnavailable)
    })?;
    Ok(Json(codes))
}

#[cfg(test)]
mod tests {
    use super::normalize_country_code;

    #[test]
    fn country_code_is_trimmed_and_uppercased() {
        assert_eq!(normalize_country_code(" br "), Some("BR".to_string()));
        assert_eq!(normalize_country_code("Us"), Some("US".to_string()));
    }

    #[test]
    fn country_code_rejects_missing_or_malformed_values() {
        for invalid in ["", "B", "BRA", "B1", "éR"] {
            assert_eq!(normalize_country_code(invalid), None);
        }
    }
}

fn authorize(user: &User, locale: &Locale) -> Result<(), ExceptionResponse> {
    if !can_manage_reference_data(user) {
        // DEF-RD-06: the refusal is about the role, not about the plan.
        return Err(ExceptionResponse::Forbidden(
            locale.clone(),
            ErrorKey::ReferenceDataForbidden,
        ));
    }
    Ok(())
}

/// DEF-RD-03/04: the same split the city endpoint makes -- a rejected file is
/// a 400 the operator can act on, in their language; an unavailable database
/// is a 500 that discloses nothing.
fn reference_failure(locale: &Locale, error: ReferenceDataError) -> ExceptionResponse {
    match error {
        ReferenceDataError::Unavailable => ExceptionResponse::InternalServerError(
            locale.clone(),
            ErrorKey::ReferenceDataUnavailable,
        ),
        rejected => {
            ExceptionResponse::BadRequestMessage(translate_reference_data_error(locale, &rejected))
        }
    }
}

fn domain(json: ProvinceJson) -> Province {
    Province {
        id: json.id,
        uuid: json.uuid,
        acronym: json.acronym,
        name: json.name,
        country_code: json.country_code,
    }
}

fn outcome(result: ImportOutcome) -> ImportResultJson {
    ImportResultJson {
        created: result.created,
        updated: result.updated,
    }
}

/// PD-027/PD-028: a página de administração de dados de referência. Distinta de
/// `GET /province`, que filtra por país e devolve a lista inteira porque
/// alimenta um seletor de endereço — paginar aquela seria truncar opções (D-9).
#[utoipa::path(
    get,
    tag = "Province",
    path = "/province/page",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of provinces. **Roles:** SysAdmin (unbound only).", body = PageJson<ProvinceJson>),
        (status = 401, body = UnauthorizedErrorJson),
        (status = 403, body = ForbiddenErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_page(
    state: State<AppState>,
    Query(page_query): Query<PageQuery>,
    Extension(locale): Extension<Locale>,
    Extension(user): Extension<User>,
) -> HttpResponse<Json<PageJson<ProvinceJson>>> {
    authorize(&user, &locale)?;
    let use_case = ProvinceUseCase::new(ProvinceGateway::new(state.conn.as_ref().clone()));
    let (page, page_size) = (page_query.page(), page_query.page_size());
    let (items, total) = use_case
        .find_page(page, page_size, page_query.search().as_deref())
        .await
        .map_err(|_| {
            ExceptionResponse::InternalServerError(locale, ErrorKey::ReferenceDataUnavailable)
        })?;
    Ok(Json(PageJson::new(
        ProvinceMapper::json_vec(items),
        page,
        page_size,
        total,
    )))
}

#[utoipa::path(
    post,
    tag = "Province",
    path = "/province",
    request_body = ProvinceJson,
    responses(
        (status = 200, description = "Province saved. **Roles:** SysAdmin (unbound only).", body = ProvinceJson),
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
    Json(payload): Json<ProvinceJson>,
) -> HttpResponse<Json<ProvinceJson>> {
    authorize(&user, &locale)?;
    let use_case = ProvinceUseCase::new(ProvinceGateway::new(state.conn.as_ref().clone()));
    use_case
        .save(domain(payload))
        .await
        .map(|saved| Json(ProvinceMapper::json(saved)))
        .map_err(|error| reference_failure(&locale, error))
}

/// PD-027: o console analisa o CSV, mostra as linhas editáveis e envia o que o
/// operador confirmou — por isso isto recebe JSON e não um arquivo. A regra é
/// tudo ou nada; um lote com qualquer linha inválida não grava nada.
#[utoipa::path(
    post,
    tag = "Province",
    path = "/province/import",
    request_body = Vec<ProvinceJson>,
    responses(
        (status = 200, description = "Import applied. **Roles:** SysAdmin (unbound only).", body = ImportResultJson),
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
    Json(payload): Json<Vec<ProvinceJson>>,
) -> HttpResponse<Json<ImportResultJson>> {
    authorize(&user, &locale)?;
    let use_case = ProvinceUseCase::new(ProvinceGateway::new(state.conn.as_ref().clone()));
    use_case
        .import(payload.into_iter().map(domain).collect())
        .await
        .map(|result| Json(outcome(result)))
        .map_err(|error| reference_failure(&locale, error))
}
