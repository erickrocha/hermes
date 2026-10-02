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

    InternalServerError(Locale, ErrorKey),

    /// A dependency outside hermes (the tracking provider) did not answer.
    ServiceUnavailable(Locale, ErrorKey),

    /// PD-027: 400 cujo corpo é uma mensagem já formada, não uma chave de
    /// i18n. Existe para a importação de dados de referência, onde o valor da
    /// resposta é *qual linha* foi recusada e por quê — detalhe por natureza
    /// dinâmico, que nenhuma chave traduzida consegue carregar. Use só quando
    /// a mensagem for o conteúdo; para erros fixos, a chave continua certa.
    ///
    /// Limitação conhecida: a mensagem sai em inglês, porque o texto vem da
    /// camada de negócio e não de um bundle. Aceitável enquanto o público é o
    /// administrador da plataforma; se um dia o operador for do tenant, os
    /// motivos precisam virar chaves com parâmetros.
    BadRequestMessage(String),
}

impl IntoResponse for ExceptionResponse {
    fn into_response(self) -> axum::http::Response<axum::body::Body> {
        if let ExceptionResponse::BadRequestMessage(message) = self {
            let payload = ErrorResponseJson::new("ImportRejected".to_string(), message);
            return (axum::http::StatusCode::BAD_REQUEST, Json(payload)).into_response();
        }

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
            ExceptionResponse::InternalServerError(locale, key) => {
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, locale, key)
            }
            ExceptionResponse::ServiceUnavailable(locale, key) => {
                (axum::http::StatusCode::SERVICE_UNAVAILABLE, locale, key)
            }
            ExceptionResponse::BadRequestMessage(..) => unreachable!("handled above"),
        };

        let payload = ErrorResponseJson::new(key.as_str().to_string(), translate(locale, key));
        (status, Json(payload)).into_response()
    }
}
