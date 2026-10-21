use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    ForbiddenErrorJson, InternalServerErrorJson, UnauthorizedErrorJson,
};
use crate::endpoints::json::tenant_dashboard_json::TenantDashboardJson;
use axum::Json;
use axum::extract::{Extension, State};
use business::domain::enums::Role;
use business::domain::user::User;
use business::gateway::tenant_dashboard_gateway::TenantDashboardGateway;
use business::use_cases::tenant_dashboard_use_case::TenantDashboardUseCase;
use chrono::Utc;

fn use_case(state: &AppState) -> TenantDashboardUseCase {
    TenantDashboardUseCase::new(TenantDashboardGateway::new(state.conn.as_ref().clone()))
}

/// An unbound SysAdmin would otherwise see every tenant's operation. This
/// overview is the caller's tenant, never the platform.
fn ensure_tenant_bound(locale: &Locale, user: &User) -> Result<(), ExceptionResponse> {
    if user.tenant_id.is_some() {
        Ok(())
    } else {
        Err(ExceptionResponse::Forbidden(
            locale.clone(),
            ErrorKey::DashboardForbidden,
        ))
    }
}

/// The tenant's own operation: fleet, workshop, yard and, for the owner, overdue charges.
#[utoipa::path(
    get,
    tag = "Dashboard",
    path = "/dashboard",
    responses(
        (status = 200, description = "Operational counts for the caller's tenant only. `overdueCharges` is present for the TenantOwner and omitted otherwise. **Roles:** TenantOwner, TenantUser, Driver, Mechanic (own tenant only). An unbound SysAdmin is refused.", body = TenantDashboardJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller is not bound to a tenant", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn snapshot(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<TenantDashboardJson>> {
    ensure_tenant_bound(&locale, &current_user)?;
    let include_billing = current_user.role == Role::TenantOwner;
    let snapshot = use_case(&state)
        .snapshot(Utc::now().naive_utc(), include_billing)
        .await
        .map_err(|_| ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError))?;
    Ok(Json(TenantDashboardJson::from(snapshot)))
}
