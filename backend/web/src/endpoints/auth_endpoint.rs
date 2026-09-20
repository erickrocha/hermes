use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::access_token_json::AccessTokenJson;
use crate::endpoints::json::login_request::LoginRequest;
use crate::endpoints::json::refresh_token_request::RefreshTokenRequest;
use crate::infrastructure::mapper::{AccessTokenMapper, Mapper};
use crate::AppState;
use axum::extract::State;
use axum::{Extension, Form, Json};
use axum::http::StatusCode;
use business::use_cases::account_invite_use_case::AccountInviteUseCase;
use business::use_cases::authentication_use_case::AuthenticationUseCase;
use business::gateway::tenant_gateway::TenantGateway;
use business::domain::access_token::AccessToken;
use business::use_cases::tenant_use_case::TenantUseCase;

/// HRMS-204/OBS-TP-05: tenants are addressed by their public uuid, so the
/// console cannot fetch the session's own tenant from the numeric id alone.
/// The uuid is resolved here rather than carried as a JWT claim, because
/// nothing authorizes on it -- `tenant_id` remains the scoping value.
async fn with_tenant_uuid(state: &AppState, mut token: AccessToken) -> AccessToken {
    if let Some(tenant_id) = token.tenant_id {
        let use_case = TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()));
        if let Ok(tenant) = use_case.find_by_id(tenant_id).await {
            token.tenant_uuid = tenant.uuid;
        }
    }
    token
}

#[utoipa::path(
    post,
    path = "/login",
    request_body(content = LoginRequest, content_type = "application/x-www-form-urlencoded"),
    responses(
        (status = 200, description = "Login successful", body = AccessTokenJson),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn sign_in(state: State<AppState>,Extension(locale): Extension<Locale>,Form(login_request): Form<LoginRequest>) -> HttpResponse<Json<AccessTokenJson>> {
    let access_token =
        AuthenticationUseCase::execute(&state.conn, login_request.email, login_request.password)
            .await;
    match access_token {
        Ok(token) => Ok(Json(AccessTokenMapper::json(
            with_tenant_uuid(&state, token).await,
        ))),
        Err(_) => Err(ExceptionResponse::Unauthorized(
            locale,
            ErrorKey::BadCredentials,
        )),
    }
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcceptInviteJson {
    pub token: String,
    pub new_password: String,
}

#[utoipa::path(
    post,
    path = "/accept-invite",
    tag = "Authentication",
    request_body = AcceptInviteJson,
    responses((status = 204, description = "Senha definida"))
)]
pub async fn accept_invite(state: State<AppState>,Extension(locale): Extension<Locale>,Json(payload): Json<AcceptInviteJson>) -> Result<StatusCode, ExceptionResponse> {
    match AccountInviteUseCase::accept(state.conn.as_ref(), &payload.token, &payload.new_password).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(_) => Err(ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)),
    }
}

#[utoipa::path(
    post,
    path = "/refresh",
    request_body = RefreshTokenRequest,
    responses(
        (status = 200, description = "Token refreshed successfully", body = AccessTokenJson),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn refresh_token(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Json(request): Json<RefreshTokenRequest>,
) -> HttpResponse<Json<AccessTokenJson>> {
    let access_token =
        AuthenticationUseCase::refresh_token(&state.conn, request.refresh_token).await;
    match access_token {
        Ok(token) => Ok(Json(AccessTokenMapper::json(
            with_tenant_uuid(&state, token).await,
        ))),
        Err(_) => Err(ExceptionResponse::Unauthorized(
            locale,
            ErrorKey::BadCredentials,
        )),
    }
}
