use crate::commons::i18n::{ErrorKey, Locale, translate};
use crate::endpoints::json::error_response_json::ErrorResponseJson;
use axum::Json;
use axum::response::IntoResponse;

pub type HttpResponse<T> = Result<T, ExceptionResponse>;

#[derive(Debug)]
pub enum ExceptionResponse {
    Unauthorized(Locale, ErrorKey),

    Forbidden(Locale, ErrorKey),

    BadRequest(Locale, ErrorKey),

    NotFound(Locale, ErrorKey),

    Conflict(Locale, ErrorKey),
}

impl IntoResponse for ExceptionResponse {
    fn into_response(self) -> axum::http::Response<axum::body::Body> {
        let (status, locale, key) = match self {
            ExceptionResponse::Unauthorized(locale, key) => {
                (axum::http::StatusCode::UNAUTHORIZED, locale, key)
            }
            ExceptionResponse::Forbidden(locale, key) => {
                (axum::http::StatusCode::FORBIDDEN, locale, key)
            }
            ExceptionResponse::BadRequest(locale, key) => {
                (axum::http::StatusCode::BAD_REQUEST, locale, key)
            }
            ExceptionResponse::NotFound(locale, key) => {
                (axum::http::StatusCode::NOT_FOUND, locale, key)
            }
            ExceptionResponse::Conflict(locale, key) => {
                (axum::http::StatusCode::CONFLICT, locale, key)
            }
        };

        let payload = ErrorResponseJson::new(key.as_str().to_string(), translate(locale, key));
        (status, Json(payload)).into_response()
    }
}
