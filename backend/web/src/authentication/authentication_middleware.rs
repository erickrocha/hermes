use crate::commons::exception_response::ExceptionResponse;
use crate::commons::i18n::{ErrorKey, Locale};
use crate::AppState;
use axum::extract::State;
use axum::http::header::{ACCEPT_LANGUAGE, AUTHORIZATION};
use axum::{body::Body, extract::Request, http::Response, middleware::Next};
use business::domain::enums::Role;
use business::use_cases::authentication_use_case::AuthenticationUseCase;

pub async fn authentication(state: State<AppState>,mut req: Request<Body>,next: Next) -> Result<Response<Body>, ExceptionResponse> {
    let locale = Locale::from_accept_language(
        req.headers()
            .get(ACCEPT_LANGUAGE)
            .and_then(|value| value.to_str().ok()),
    );
    req.extensions_mut().insert(locale);

    if req.uri().path().starts_with("/login")
        || req.uri().path().starts_with("/signup")
        || req.uri().path().starts_with("/refresh")
        || req.uri().path().starts_with("/accept-invite")
        || (req.method() == axum::http::Method::GET && req.uri().path() == "/legal/documents")
    {
        return Ok(next.run(req).await);
    }

    let auth_header = req.headers_mut().get(AUTHORIZATION);
    let auth_header = match auth_header {
        Some(header) => header
            .to_str()
            .map_err(|_| ExceptionResponse::Forbidden(locale, ErrorKey::AuthHeaderMissing))?,
        None => {
            return Err(ExceptionResponse::Forbidden(
                locale,
                ErrorKey::RequiredHeaderValueMissing,
            ))
        }
    };

    let mut header = auth_header.split_whitespace();

    let (bearer, token) = (header.next(), header.next());

    if bearer != Some("Bearer") || token.is_none() {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::InvalidJwtToken,
        ));
    }

    let current_user = AuthenticationUseCase::validate(&state.conn, token.unwrap().to_string())
        .await
        .map_err(|_| ExceptionResponse::Unauthorized(locale, ErrorKey::BadCredentials))?;

    if matches!(current_user.role, Role::TenantOwner | Role::TenantUser) && current_user.tenant_id.is_none() {
        return Err(ExceptionResponse::Forbidden(locale,ErrorKey::InvalidParameterValue,));
    }

    let audit_user = entity::audit::AuditUser {
        id: current_user.id.unwrap_or(0),
        email: current_user.email.clone(),
        tenant_id: current_user.tenant_id,
        enforce_tenant: current_user.tenant_id.is_some() || current_user.role != Role::SysAdmin,
    };
    
    req.extensions_mut().insert(current_user);
    Ok(entity::audit::run_with_user(Some(audit_user), next.run(req)).await)
}
