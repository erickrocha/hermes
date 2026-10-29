use crate::AppState;
use crate::commons::exception_response::ExceptionResponse;
use crate::commons::i18n::{ErrorKey, Locale};
use axum::extract::{MatchedPath, OriginalUri, State};
use axum::http::header::{ACCEPT_LANGUAGE, AUTHORIZATION};
use axum::{body::Body, extract::Request, http::Response, middleware::Next};
use business::domain::authorization::{is_platform_actor, lacks_required_tenant};
use business::domain::permission;
use business::domain::cross_tenant_action::CrossTenantAction;
use business::domain::tenant_tree;
use business::gateway::cross_tenant_action_gateway::CrossTenantActionGateway;
use business::use_cases::cross_tenant_action_use_case::CrossTenantActionUseCase;
use business::gateway::tenant_gateway::{TenantGateway, TenantStore};
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

    let mut current_user = current_user;
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
    // `C-036` (`DS-1`): a request may act inside a descendant tenant by naming it in `X-Tenant-Uuid`. The
    // caller becomes, for this request, a member of that tenant holding what their `tenant.child.*`
    // permissions stand for there; everything downstream then works unchanged, tenant-confined.
    let delegated: Option<bool> = match req.headers().get(TARGET_TENANT_HEADER).map(|v| v.to_str().map(str::to_string)) {
        None => None,
        Some(Err(_)) => return Err(ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)),
        Some(Ok(uuid)) => {
            let method = req.method().as_str().to_string();
            let path = req.extensions().get::<MatchedPath>().map(|p| p.as_str().to_string()).unwrap_or_default();
            let concrete = original_path(&req).to_string();
            enter_descendant(&state, &locale, &mut current_user, uuid.trim(), &method, &path, &concrete).await?
        }
    };
    if let Some(tenant_id) = current_user.bound_tenant() {
        ensure_billing_active(&state, &locale, tenant_id).await?;
    }

    let audit_user = entity::audit::AuditUser {
        id: user_id,
        email: current_user.email.clone(),
        tenant_id: current_user.bound_tenant(),
        enforce_tenant: delegated.is_some() || !is_platform_actor(&current_user),
    };

    // `HRMS-853`/`HRMS-856` (`C-035`): every operation names the permission it asks for, and every caller
    // is held to their effective grants -- profile, extras and baseline, resolved with the account. An
    // operation not in the table (the dashboard, `/access-profile/mine`) asks only for a session. The
    // handler still asks the tenant question with the record in hand (`authorization`).
    if let Some(path) = req.extensions().get::<MatchedPath>()
        && let Some(code) = permission::for_operation(req.method().as_str(), path.as_str())
        && !current_user.allows(code)
    {
        if let Some(write_without_read) = delegated {
            // Inside a descendant the refusal names the `tenant.child.*` permission that would stand for the route
            // -- and when the caller holds the write but not the read, the read they are missing.
            let child = if write_without_read {
                permission::CHILD_DATA_READ
            } else {
                permission::child_permission_for(code, req.method().as_str())
            };
            return Err(ExceptionResponse::ForbiddenPermission(locale, ErrorKey::PermissionNotInProfile, child));
        }
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::PermissionNotInProfile));
    }

    req.extensions_mut().insert(current_user);
    Ok(entity::audit::run_with_user(Some(audit_user), next.run(req)).await)
}

const TARGET_TENANT_HEADER: &str = "x-tenant-uuid";

/// Reads the console makes by itself on every screen -- the session's permissions, the notification badge's poll,
/// and the trail's own listing -- are not something done *to the tenant's data*, so they leave no trace; they would
/// bury the actions that matter. Everything else, read or write, is recorded.
const UNAUDITED_READS: [&str; 3] = ["/access-profile/mine", "/maintenance-notification/unread-count", "/cross-tenant-action"];

fn is_audited(method: &str, route: &str) -> bool {
    !(method == "GET" && UNAUDITED_READS.contains(&route))
}

/// `DS-1`/`DS-3` (`SR-AC-08-03`): swaps `user` for a member of the descendant named by `uuid`. Naming the caller's
/// own tenant changes nothing (`false`); a tenant that is not below the caller's is a 404, as for any tenant
/// out of reach (`PD-034`); a caller with no `tenant.child.*` permission is refused naming the one to grant.
/// Returns `None` when the request still acts in the caller's own tenant, else `Some(write_without_read)`:
/// whether the caller holds `tenant.child.data.write` without the `data.read` it needs.
async fn enter_descendant(
    state: &AppState,
    locale: &Locale,
    user: &mut business::domain::user::User,
    uuid: &str,
    method: &str,
    path: &str,
    concrete_path: &str,
) -> Result<Option<bool>, ExceptionResponse> {
    let not_found = || ExceptionResponse::NotFound(locale.clone(), ErrorKey::TenantNotFound);
    let Some(own) = user.tenant_id else { return Err(not_found()) };
    let tenants = entity::audit::run_as_platform(TenantGateway::new(state.conn.as_ref().clone()).list_all())
        .await
        .map_err(|_| ExceptionResponse::InternalServerError(locale.clone(), ErrorKey::UnexpectedError))?;
    let target = tenants.iter().find(|t| t.uuid.as_deref() == Some(uuid)).ok_or_else(not_found)?;
    let target_id = target.id.ok_or_else(not_found)?;
    if target_id == own {
        return Ok(None);
    }
    if !tenant_tree::is_descendant(&tenants, own, target_id) {
        return Err(not_found());
    }
    let Some(grants) = permission::child_context_grants(&user.access.grants) else {
        // Writing without reading is the one case where a held permission is what is missing.
        let wanted = if user.access.grants.allows(permission::CHILD_DATA_WRITE) {
            permission::CHILD_DATA_READ
        } else {
            permission::child_permission_for(permission::for_operation(method, path).unwrap_or(""), method)
        };
        return Err(ExceptionResponse::ForbiddenPermission(locale.clone(), ErrorKey::PermissionNotInProfile, wanted));
    };
    let write_without_read = user.access.grants.allows(permission::CHILD_DATA_WRITE) && !user.access.grants.allows(permission::CHILD_DATA_READ);
    user.access.acting_from = Some(own);
    user.tenant_id = Some(target_id);
    user.access.scope = business::domain::enums::ProfileScope::Tenant;
    user.access.grants = grants;
    if !is_audited(method, path) {
        return Ok(Some(write_without_read));
    }
    // `HRMS-877`/`SR-AC-08-06`: the trail is written before the request runs -- if it cannot be, nothing is done.
    let actor_user_id = user.id.unwrap_or_default();
    let action = CrossTenantAction {
        id: None,
        uuid: None,
        actor_user_id,
        actor_email: user.email.clone(),
        actor_tenant_id: own,
        affected_tenant_id: target_id,
        kind: CrossTenantAction::kind_of(method).to_string(),
        method: method.to_string(),
        route: path.to_string(),
        path: concrete_path.to_string(),
        occurred_at: chrono::Utc::now().naive_utc(),
    };
    CrossTenantActionUseCase::new(CrossTenantActionGateway::new(state.conn.as_ref().clone()))
        .record(action)
        .await
        .map_err(|_| ExceptionResponse::InternalServerError(locale.clone(), ErrorKey::UnexpectedError))?;
    Ok(Some(write_without_read))
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
            | "/garage-attendance/monitor/map"
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
        assert!(is_device_monitor_path(&Method::GET, "/garage-attendance/monitor/map"));
        assert!(!is_device_monitor_path(&Method::POST, "/garage-attendance/monitor"));
        assert!(!is_device_monitor_path(&Method::GET, "/garage-attendance/queue"));
        assert!(!is_device_monitor_path(&Method::GET, "/garage-device"));
    }
}

#[cfg(test)]
mod audit_tests {
    use super::is_audited;

    /// `HRMS-877`: the console's own housekeeping reads leave no trace; everything else does, read or write.
    #[test]
    fn only_the_consoles_housekeeping_reads_go_unrecorded() {
        assert!(!is_audited("GET", "/access-profile/mine"));
        assert!(!is_audited("GET", "/maintenance-notification/unread-count"));
        assert!(!is_audited("GET", "/cross-tenant-action"));
        assert!(is_audited("GET", "/vehicle"));
        assert!(is_audited("GET", "/tenant/uuid/{uuid}"));
        assert!(is_audited("PUT", "/vehicle/uuid/{uuid}"));
        assert!(is_audited("POST", "/access-profile/mine"), "only reads are exempt");
    }
}
