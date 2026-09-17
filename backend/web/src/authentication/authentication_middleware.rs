use crate::commons::exception_response::ExceptionResponse;
use crate::commons::i18n::{ErrorKey, Locale};
use crate::AppState;
use axum::extract::State;
use axum::http::header::{ACCEPT_LANGUAGE, AUTHORIZATION};
use axum::{body::Body, extract::Request, http::Response, middleware::Next};
use business::domain::authorization::is_unbound_sys_admin;
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

    // EPIC-IA-06-S02 (HRMS-120): a user created with a caller-supplied
    // password (PD-019's TenantOwner-creates-TenantUser path, and any admin
    // creation) must set their own before reaching anything else. Before
    // this, `first_login` was carried in the token and enforced only by the
    // backoffice's `/first-access` redirect -- a UX affordance a bearer
    // token and curl bypass entirely, the exact gap PD-015 already closed
    // for business-plan administration.
    if current_user.first_login && !allowed_before_password_is_set(req.method(), req.uri().path()) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::PasswordChangeRequired));
    }

    let audit_user = entity::audit::AuditUser {
        id: current_user.id.unwrap_or(0),
        email: current_user.email.clone(),
        tenant_id: current_user.tenant_id,
        enforce_tenant: !is_unbound_sys_admin(&current_user),
    };
    
    req.extensions_mut().insert(current_user);
    Ok(entity::audit::run_with_user(Some(audit_user), next.run(req)).await)
}

/// The one route a first-login user may reach: the endpoint that lets them
/// stop being one.
fn allowed_before_password_is_set(method: &axum::http::Method, path: &str) -> bool {
    method == axum::http::Method::PUT && path == "/user/change-password"
}

#[cfg(test)]
mod tests {
    use super::allowed_before_password_is_set;
    use axum::http::Method;

    #[test]
    fn allows_only_put_change_password() {
        assert!(allowed_before_password_is_set(&Method::PUT, "/user/change-password"));
    }

    #[test]
    fn rejects_other_methods_on_the_same_path() {
        assert!(!allowed_before_password_is_set(&Method::GET, "/user/change-password"));
        assert!(!allowed_before_password_is_set(&Method::POST, "/user/change-password"));
    }

    #[test]
    fn rejects_every_other_path() {
        assert!(!allowed_before_password_is_set(&Method::PUT, "/user/1"));
        assert!(!allowed_before_password_is_set(&Method::GET, "/tenant"));
        assert!(!allowed_before_password_is_set(&Method::GET, "/business-plan"));
    }
}
