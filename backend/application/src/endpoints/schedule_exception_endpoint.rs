use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::json::schedule_exception_json::ScheduleExceptionJson;
use crate::endpoints::transport_demand_endpoint::{find_visible, resolve_driver_id, resolve_vehicle_id};
use crate::infrastructure::mapper::reject_unknown_schedule_exception_type;
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::can_administer_transport_demand;
use business::domain::enums::ScheduleExceptionType;
use business::domain::schedule_exception::ScheduleException;
use business::domain::transport_demand::TransportDemand;
use business::domain::user::User;
use business::gateway::schedule_exception_gateway::ScheduleExceptionGateway;
use business::gateway::user_gateway::UserGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::use_cases::schedule_exception_use_case::{
    NOT_A_DRIVER, NOT_A_TENANT_VEHICLE, ScheduleExceptionUseCase,
};
use business::use_cases::user_use_case::UserUseCase;
use business::use_cases::vehicle_use_case::VehicleUseCase;
use std::str::FromStr;

fn use_case(state: &AppState) -> ScheduleExceptionUseCase {
    let db = state.conn.as_ref().clone();
    ScheduleExceptionUseCase::new(
        ScheduleExceptionGateway::new(db.clone()),
        UserGateway::new(db.clone()),
        VehicleGateway::new(db),
    )
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        NOT_A_DRIVER => ExceptionResponse::BadRequest(locale, ErrorKey::NotADriver),
        NOT_A_TENANT_VEHICLE => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

fn require_administrator(
    locale: &Locale,
    user: &User,
    demand: &TransportDemand,
) -> Result<(), ExceptionResponse> {
    if can_administer_transport_demand(user, demand.tenant_id) {
        Ok(())
    } else {
        Err(ExceptionResponse::Forbidden(
            locale.clone(),
            ErrorKey::TransportDemandForbidden,
        ))
    }
}

async fn json(state: &AppState, exception: ScheduleException) -> ScheduleExceptionJson {
    let new_driver_uuid = match exception.new_driver_id {
        Some(id) => UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()))
            .find_by_id(id)
            .await
            .ok()
            .and_then(|u| u.uuid),
        None => None,
    };
    let new_vehicle_uuid = match exception.new_vehicle_id {
        Some(id) => VehicleUseCase::new(VehicleGateway::new(state.conn.as_ref().clone()))
            .find_by_id(id)
            .await
            .ok()
            .and_then(|v| v.uuid),
        None => None,
    };

    ScheduleExceptionJson {
        uuid: exception.uuid,
        date: exception.date,
        exception_type: exception.exception_type.to_string(),
        new_driver_uuid,
        new_vehicle_uuid,
        reason: exception.reason,
        extra_trip_id: exception.extra_trip_id,
        status: exception.status,
    }
}

#[utoipa::path(
    post,
    tag = "Transport Demand",
    path = "/transport-demand/uuid/{uuid}/exception",
    params(("uuid" = String, Path, description = "Transport demand UUID")),
    request_body = ScheduleExceptionJson,
    responses(
        (status = 201, description = "The day exception is recorded (HRMS-606). **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only).", body = ScheduleExceptionJson),
        (status = 400, description = "Bad request, including an exception type outside Cancellation/Deallocation/Substitution (HRMS-606), `NotADriver`, or a vehicle outside the demand's own tenant", body = BadRequestErrorJson),
        (status = 404, description = "Demand not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller may read this demand but not record an exception against it", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<ScheduleExceptionJson>,
) -> HttpResponse<(StatusCode, Json<ScheduleExceptionJson>)> {
    let demand = find_visible(&state, &locale, &current_user, uuid).await?;
    require_administrator(&locale, &current_user, &demand)?;
    reject_unknown_schedule_exception_type(&payload.exception_type, &locale)?;

    let new_driver_id = resolve_driver_id(&state, &locale, payload.new_driver_uuid.clone()).await?;
    let new_vehicle_id = resolve_vehicle_id(&state, &locale, payload.new_vehicle_uuid.clone()).await?;

    let domain = ScheduleException {
        id: None,
        uuid: None,
        tenant_id: demand.tenant_id,
        demand_id: demand.id.unwrap_or_default(),
        date: payload.date,
        // `reject_unknown_schedule_exception_type` already refused an
        // unrecognised value; this never reaches the fallback default.
        exception_type: ScheduleExceptionType::from_str(&payload.exception_type)
            .unwrap_or(ScheduleExceptionType::Cancellation),
        new_driver_id,
        new_vehicle_id,
        reason: payload.reason,
        extra_trip_id: payload.extra_trip_id,
        status: payload.status,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };

    match use_case(&state).create(domain).await {
        Ok(exception) => Ok((StatusCode::CREATED, Json(json(&state, exception).await))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "Transport Demand",
    path = "/transport-demand/uuid/{uuid}/exceptions",
    params(("uuid" = String, Path, description = "Transport demand UUID"), PageQuery),
    responses(
        (status = 200, description = "The demand's day exceptions, newest first (PD-028). **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = PageJson<ScheduleExceptionJson>),
        (status = 404, description = "Demand not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn history(
    state: State<AppState>,
    Query(page_query): Query<PageQuery>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<PageJson<ScheduleExceptionJson>>> {
    let demand = find_visible(&state, &locale, &current_user, uuid).await?;
    let (page, page_size) = (page_query.page(), page_query.page_size());
    match use_case(&state)
        .history(demand.id.unwrap_or_default(), page, page_size)
        .await
    {
        Ok((items, total)) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(json(&state, item).await);
            }
            Ok(Json(PageJson::new(out, page, page_size, total)))
        }
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    delete,
    tag = "Transport Demand",
    path = "/transport-demand/uuid/{uuid}/exception/uuid/{exception_uuid}",
    params(
        ("uuid" = String, Path, description = "Transport demand UUID"),
        ("exception_uuid" = String, Path, description = "Exception UUID"),
    ),
    responses(
        (status = 204, description = "The exception is removed. **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only)."),
        (status = 404, description = "`TransportDemandNotFound` (also for another tenant's demand, PD-034), or the exception does not belong to this demand", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller may read this demand but not remove an exception against it", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn remove(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path((uuid, exception_uuid)): Path<(String, String)>,
) -> HttpResponse<StatusCode> {
    let demand = find_visible(&state, &locale, &current_user, uuid).await?;
    require_administrator(&locale, &current_user, &demand)?;

    match use_case(&state)
        .remove(demand.id.unwrap_or_default(), exception_uuid)
        .await
    {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(_) => Err(ExceptionResponse::NotFound(
            locale,
            ErrorKey::TransportDemandNotFound,
        )),
    }
}
