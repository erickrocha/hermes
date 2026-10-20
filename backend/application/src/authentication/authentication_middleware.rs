use crate::AppState;
use crate::commons::exception_response::ExceptionResponse;
use crate::commons::i18n::{ErrorKey, Locale};
use axum::extract::{MatchedPath, OriginalUri, State};
use axum::http::header::{ACCEPT_LANGUAGE, AUTHORIZATION};
use axum::{body::Body, extract::Request, http::Response, middleware::Next};
use business::domain::authorization::{is_unbound_sys_admin, lacks_required_tenant};
use business::domain::permission;
use business::gateway::access_profile_gateway::AccessProfileGateway;
use business::use_cases::access_profile_use_case::AccessProfileUseCase;
use business::gateway::user_gateway::UserGateway;
use business::use_cases::authentication_use_case::AuthenticationUseCase;
use business::gateway::garage_device_gateway::GarageDeviceGateway;
use business::use_cases::garage_device_use_case::GarageDeviceUseCase;
use business::gateway::billing_gateway::{BillingGateway, BillingStore};

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

    if is_public_request(&req) {
        return Ok(next.run(req).await);
    }

    let auth_header = req.headers().get(AUTHORIZATION);
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

    let token = bearer_token(auth_header)
        .ok_or_else(|| ExceptionResponse::Forbidden(locale.clone(), ErrorKey::InvalidJwtToken))?;

    let current_user = AuthenticationUseCase::new(UserGateway::new(state.conn.as_ref().clone()))
        .validate(token.to_string())
        .await;
    let current_user = match current_user {
        Ok(user) => user,
        Err(_) if is_device_monitor_request(&req) => {
            let device = GarageDeviceUseCase::new(GarageDeviceGateway::new(state.conn.as_ref().clone()))
                .validate_credential(token.to_string())
                .await
                .map_err(|_| ExceptionResponse::Unauthorized(locale.clone(), ErrorKey::BadCredentials))?;
            ensure_billing_active(&state, &locale, device.tenant_id).await?;
            let audit_user = entity::audit::AuditUser {
                id: 0,
                email: format!("device:{}", device.uuid),
                tenant_id: Some(device.tenant_id),
                enforce_tenant: true,
            };
            req.extensions_mut().insert(device);
            return Ok(entity::audit::run_with_user(Some(audit_user), next.run(req)).await);
        }
        Err(_) => {
            return Err(ExceptionResponse::Unauthorized(locale, ErrorKey::BadCredentials));
        }
    };

    let user_id = current_user
        .id
        .filter(|id| *id > 0)
        .ok_or_else(|| ExceptionResponse::Unauthorized(locale.clone(), ErrorKey::BadCredentials))?;

    if lacks_required_tenant(&current_user) {
        return Err(ExceptionResponse::Forbidden(
            locale.clone(),
            ErrorKey::InvalidParameterValue,
        ));
    }
    if let Some(tenant_id) = current_user.tenant_id {
        ensure_billing_active(&state, &locale, tenant_id).await?;
    }

    let audit_user = entity::audit::AuditUser {
        id: user_id,
        email: current_user.email.clone(),
        tenant_id: current_user.tenant_id,
        enforce_tenant: !is_unbound_sys_admin(&current_user),
    };

    // `HRMS-853` (`AC-02-S02`): a user who holds an access profile is held to it -- the operation's permission
    // must be in the profile. A user with no profile is untouched (the role's own checks answer, `IA-100`),
    // and an operation whose permission is owner-only, platform or unlisted is left to those checks too.
    if permission::takes_profile(&current_user.role)
        && let Some(path) = req.extensions().get::<MatchedPath>()
        && let Some(code) = permission::for_operation(req.method().as_str(), path.as_str())
        && permission::find(code).is_some()
    {
        // Under the caller's own tenant scope: the profile is tenant-owned and unreadable without one.
        let effective = entity::audit::run_with_user(
            Some(audit_user.clone()),
            AccessProfileUseCase::new(AccessProfileGateway::new(state.conn.as_ref().clone())).effective(&current_user),
        )
        .await
        .map_err(|_| ExceptionResponse::InternalServerError(locale.clone(), ErrorKey::UnexpectedError))?;
        if effective.profile_uuid.is_some() && !effective.permissions.iter().any(|c| c == code) {
            return Err(ExceptionResponse::Forbidden(locale, ErrorKey::PermissionNotInProfile));
        }
    }

    req.extensions_mut().insert(current_user);
    Ok(entity::audit::run_with_user(Some(audit_user), next.run(req)).await)
}

async fn ensure_billing_active(
    state: &AppState,
    locale: &Locale,
    tenant_id: i64,
) -> Result<(), ExceptionResponse> {
    let billing = BillingGateway::new(state.conn.as_ref().clone());
    let settings = entity::audit::run_for_tenant(
        tenant_id,
        "billing-access-check",
        billing.get_settings(tenant_id),
    )
    .await
    .map_err(|_| ExceptionResponse::InternalServerError(locale.clone(), ErrorKey::UnexpectedError))?;
    if settings.is_some_and(|settings| settings.suspended_at.is_some()) {
        return Err(ExceptionResponse::Forbidden(
            locale.clone(),
            ErrorKey::BillingTenantSuspended,
        ));
    }
    Ok(())
}

fn bearer_token(header: &str) -> Option<&str> {
    let mut parts = header.split_whitespace();
    match (parts.next(), parts.next()) {
        (Some("Bearer"), Some(token)) => Some(token),
        _ => None,
    }
}

fn is_device_monitor_request(req: &Request<Body>) -> bool {
    let path = original_path(req);
    is_device_monitor_path(req.method(), path)
}

fn is_public_request(req: &Request<Body>) -> bool {
    is_public_path(original_path(req))
}

fn original_path(req: &Request<Body>) -> &str {
    req.extensions().get::<OriginalUri>()
        .map(|original| original.0.path())
        .unwrap_or_else(|| req.uri().path())
}

fn is_device_monitor_path(method: &axum::http::Method, path: &str) -> bool {
    *method == axum::http::Method::GET && matches!(path,
        "/garage-attendance/monitor"
            | "/garage-attendance/monitor/cards"
            | "/garage-attendance/call-list"
            | "/garage-attendance/monitor/matrix"
    )
}

/// EPIC-XF-03-S01/S02/S03 (HRMS-018, HRMS-019, HRMS-020, D-6, U-013): the
/// authenticated surface is the default; this is the whole exception list,
/// matched exactly rather than by prefix (`/login-anything` is not public).
/// `/signup` and `GET /legal/documents` were removed rather than fixed
/// forward -- the owner confirmed both were leftovers with no route behind
/// them (D-6, U-013) -- so this set now matches the router in
/// `routes/authentication_routes.rs` exactly (HRMS-018).
fn is_public_path(path: &str) -> bool {
    matches!(path, "/login" | "/refresh" | "/accept-invite" | "/garage-device/pair")
}

#[cfg(test)]
mod tests {
    use super::{bearer_token, is_device_monitor_path, is_public_path, is_public_request};
    use axum::body::Body;
    use axum::extract::OriginalUri;
    use axum::http::Uri;

    #[test]
    fn bearer_token_requires_scheme_and_token_without_panicking() {
        assert_eq!(bearer_token("Bearer abc.def.ghi"), Some("abc.def.ghi"));
        assert_eq!(bearer_token(""), None);
        assert_eq!(bearer_token("Bearer"), None);
        assert_eq!(bearer_token("Basic abc"), None);
    }

    #[test]
    fn public_paths_match_the_router_exactly() {
        assert!(is_public_path("/login"));
        assert!(is_public_path("/refresh"));
        assert!(is_public_path("/accept-invite"));
        assert!(is_public_path("/garage-device/pair"));
    }

    #[test]
    fn a_nested_public_pair_route_uses_its_original_uri() {
        let mut request = axum::http::Request::new(Body::empty());
        request.extensions_mut().insert(OriginalUri(Uri::from_static("/garage-device/pair")));
        assert!(is_public_request(&request));
    }

    #[test]
    fn matches_exactly_not_by_prefix() {
        assert!(!is_public_path("/login-anything"));
        assert!(!is_public_path("/loginx"));
        assert!(!is_public_path("/refreshed"));
        assert!(!is_public_path("/garage-device/pairing-code"));
    }

    #[test]
    fn signup_and_legal_documents_are_not_public_they_were_leftovers() {
        assert!(!is_public_path("/signup"));
        assert!(!is_public_path("/legal/documents"));
    }

    #[test]
    fn device_credentials_are_limited_to_read_only_wall_monitor_routes() {
        use axum::http::Method;
        assert!(is_device_monitor_path(&Method::GET, "/garage-attendance/monitor"));
        assert!(is_device_monitor_path(&Method::GET, "/garage-attendance/monitor/cards"));
        assert!(is_device_monitor_path(&Method::GET, "/garage-attendance/call-list"));
        assert!(is_device_monitor_path(&Method::GET, "/garage-attendance/monitor/matrix"));
        assert!(!is_device_monitor_path(&Method::POST, "/garage-attendance/monitor"));
        assert!(!is_device_monitor_path(&Method::GET, "/garage-attendance/queue"));
        assert!(!is_device_monitor_path(&Method::GET, "/garage-device"));
    }
}
