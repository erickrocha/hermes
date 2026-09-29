use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::internal_tank_json::{InternalTankJson, InternalTankQuery};
use axum::Json;
use axum::extract::{Extension, Query, State};
use business::domain::authorization::{can_configure_internal_tank, can_read_internal_tank};
use business::domain::internal_tank::InternalTank;
use business::domain::user::User;
use business::gateway::internal_tank_gateway::InternalTankGateway;
use business::use_cases::internal_tank_use_case::{
    ALERT_LEVEL_MUST_NOT_BE_NEGATIVE, CAPACITY_MUST_BE_POSITIVE, INTERNAL_TANK_NOT_CONFIGURED,
    InternalTankUseCase, REFERENCE_STOCK_OUT_OF_RANGE, RESERVE_LEVEL_MUST_NOT_BE_NEGATIVE,
};

fn use_case(state: &AppState) -> InternalTankUseCase {
    InternalTankUseCase::new(InternalTankGateway::new(state.conn.as_ref().clone()))
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        CAPACITY_MUST_BE_POSITIVE
        | REFERENCE_STOCK_OUT_OF_RANGE
        | ALERT_LEVEL_MUST_NOT_BE_NEGATIVE
        | RESERVE_LEVEL_MUST_NOT_BE_NEGATIVE => {
            ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)
        }
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

fn json(tank: InternalTank) -> InternalTankJson {
    InternalTankJson {
        uuid: tank.uuid,
        tenant_id: tank.tenant_id,
        capacity_liters: tank.capacity_liters,
        reference_stock_liters: tank.reference_stock_liters,
        reference_at: tank.reference_at,
        alert_level_liters: tank.alert_level_liters,
        reserve_level_liters: tank.reserve_level_liters,
    }
}

#[utoipa::path(
    put,
    tag = "InternalTank",
    path = "/internal-tank",
    request_body = InternalTankJson,
    responses(
        (status = 200, description = "The caller's own tenant's internal tank is configured (TRM-1540) -- created if this is the first configuration, updated in place otherwise, since a fresh reading replaces the manager's last one rather than appending to a ledger. **Roles:** SysAdmin (unbound; names the owning tenant); TenantOwner (own tenant only).", body = InternalTankJson),
        (status = 400, description = "Bad request, including a non-positive capacity, a reference stock outside [0, capacity], or a negative alert/reserve level", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not configure the internal tank", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn configure(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<InternalTankJson>,
) -> HttpResponse<Json<InternalTankJson>> {
    if !can_configure_internal_tank(&current_user, payload.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::InternalTankForbidden));
    }

    let mut tenant_id = payload.tenant_id;
    if current_user.tenant_id.is_some() {
        tenant_id = current_user.tenant_id;
    }

    let tank = InternalTank {
        id: None,
        uuid: None,
        tenant_id,
        capacity_liters: payload.capacity_liters,
        reference_stock_liters: payload.reference_stock_liters,
        reference_at: payload.reference_at,
        alert_level_liters: payload.alert_level_liters,
        reserve_level_liters: payload.reserve_level_liters,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };

    match use_case(&state).configure(tank).await {
        Ok(saved) => Ok(Json(json(saved))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "InternalTank",
    path = "/internal-tank",
    params(InternalTankQuery),
    responses(
        (status = 200, description = "The caller's own tenant's internal tank configuration. `tenantId` names the tenant for an unbound `SysAdmin`; ignored for a tenant-bound caller. **Roles:** SysAdmin, TenantOwner, TenantUser, Driver, Mechanic (any authenticated role; own tenant only).", body = InternalTankJson),
        (status = 404, description = "The internal tank has not been configured for this tenant (TRM-1543)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_current(
    state: State<AppState>,
    Query(query): Query<InternalTankQuery>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<InternalTankJson>> {
    let target_tenant_id = current_user.tenant_id.or(query.tenant_id);
    let tank = use_case(&state)
        .get_current(target_tenant_id)
        .await
        .map_err(|e| {
            if e.message == INTERNAL_TANK_NOT_CONFIGURED {
                ExceptionResponse::NotFound(locale.clone(), ErrorKey::InternalTankNotConfigured)
            } else {
                ExceptionResponse::InternalServerError(locale.clone(), ErrorKey::UnexpectedError)
            }
        })?;
    if !can_read_internal_tank(&current_user, tank.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::InternalTankNotConfigured));
    }
    Ok(Json(json(tank)))
}
