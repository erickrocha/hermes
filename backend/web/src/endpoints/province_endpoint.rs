use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::infrastructure::mapper::{Mapper, ProvinceMapper};
use axum::Json;
use axum::extract::Extension;
use axum::extract::{Path, Query, State};
use business::gateway::province_gateway::ProvinceGateway;
use business::use_cases::province_use_case::ProvinceUseCase;
use crate::endpoints::json::error_response_json::{BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson, UnauthorizedErrorJson};
use crate::endpoints::json::province_json::ProvinceJson;

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ProvinceQueryParams {
    /// ISO 3166-1 alpha-2 country code.
    #[serde(alias = "country_code")]
    pub country_code: String,
}

fn normalize_country_code(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_uppercase();
    (normalized.len() == 2 && normalized.bytes().all(|byte| byte.is_ascii_alphabetic()))
        .then_some(normalized)
}

#[utoipa::path(
    get,
    tag = "Province",
    path = "/province",
    params(ProvinceQueryParams),
    responses(
        (status = 200, description = "List of provinces", body = Vec<ProvinceJson>),
        (status = 400, description = "Missing or invalid country code", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_all(state: State<AppState>,Extension(locale): Extension<Locale>,Query(params): Query<ProvinceQueryParams>) -> HttpResponse<Json<Vec<ProvinceJson>>> {
    let country_code = normalize_country_code(&params.country_code)
        .ok_or(ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue))?;

    let use_case = ProvinceUseCase::new(ProvinceGateway::new(state.conn.as_ref().clone()));
    match use_case.find_by_country_code(country_code).await {
        Ok(list) => Ok(Json(ProvinceMapper::json_vec(list))),
        Err(_) => Ok(Json(Vec::new())),
    }
}

#[utoipa::path(
    get,
    tag = "Province",
    path = "/province/{id}",
    params(
        ("id" = i32, Path, description = "Province ID")
    ),
    responses(
        (status = 200, description = "Province found", body = ProvinceJson),
        (status = 404, description = "Province not found", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_by_id(state: State<AppState>,Extension(locale): Extension<Locale>,Path(id): Path<i64>) -> HttpResponse<Json<ProvinceJson>> {
    let use_case = ProvinceUseCase::new(ProvinceGateway::new(state.conn.as_ref().clone()));
    match use_case.find_by_id(id).await {
        Ok(res) => Ok(Json(ProvinceMapper::json(res))),
        Err(_) => Err(ExceptionResponse::NotFound(
            locale,
            ErrorKey::RequiredParameterMissing,
        )),
    }
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
