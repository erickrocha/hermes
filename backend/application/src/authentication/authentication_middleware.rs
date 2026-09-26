use crate::AppState;
use crate::commons::exception_response::ExceptionResponse;
use crate::commons::i18n::{ErrorKey, Locale};
use axum::extract::State;
use axum::http::header::{ACCEPT_LANGUAGE, AUTHORIZATION};
use axum::{body::Body, extract::Request, http::Response, middleware::Next};
use business::domain::authorization::{is_unbound_sys_admin, lacks_required_tenant};
use business::use_cases::authentication_use_case::AuthenticationUseCase;

pub async fn authentication(
    state: State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response<Body>, ExceptionResponse> {
    let locale = Locale::from_accept_language(
        req.headers()
            .get(ACCEPT_LANGUAGE)
            .and_then(|value| value.to_str().ok()),
    );
    req.extensions_mut().insert(locale.clone());

    if is_public_path(req.uri().path()) {
        return Ok(next.run(req).await);
    }

    let auth_header = req.headers_mut().get(AUTHORIZATION);
    let auth_header = match auth_header {
        Some(header) => header.to_str().map_err(|_| {
            ExceptionResponse::Forbidden(locale.clone(), ErrorKey::AuthHeaderMissing)
        })?,
        None => {
            return Err(ExceptionResponse::Forbidden(
                locale,
                ErrorKey::RequiredHeaderValueMissing,
            ));
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
        .map_err(|_| ExceptionResponse::Unauthorized(locale.clone(), ErrorKey::BadCredentials))?;

    if lacks_required_tenant(&current_user) {
        return Err(ExceptionResponse::Forbidden(
            locale.clone(),
            ErrorKey::InvalidParameterValue,
        ));
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

/// EPIC-XF-03-S01/S02/S03 (HRMS-018, HRMS-019, HRMS-020, D-6, U-013): the
/// authenticated surface is the default; this is the whole exception list,
/// matched exactly rather than by prefix (`/login-anything` is not public).
/// `/signup` and `GET /legal/documents` were removed rather than fixed
/// forward -- the owner confirmed both were leftovers with no route behind
/// them (D-6, U-013) -- so this set now matches the router in
/// `routes/authentication_routes.rs` exactly (HRMS-018).
fn is_public_path(path: &str) -> bool {
    matches!(path, "/login" | "/refresh" | "/accept-invite")
}

#[cfg(test)]
mod tests {
    use super::is_public_path;

    #[test]
    fn public_paths_match_the_router_exactly() {
        assert!(is_public_path("/login"));
        assert!(is_public_path("/refresh"));
        assert!(is_public_path("/accept-invite"));
    }

    #[test]
    fn matches_exactly_not_by_prefix() {
        assert!(!is_public_path("/login-anything"));
        assert!(!is_public_path("/loginx"));
        assert!(!is_public_path("/refreshed"));
    }

    #[test]
    fn signup_and_legal_documents_are_not_public_they_were_leftovers() {
        assert!(!is_public_path("/signup"));
        assert!(!is_public_path("/legal/documents"));
    }
}
