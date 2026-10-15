use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::daily_schedule_json::DailyScheduleJson;
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::transport_demand_endpoint::{find_visible, resolve_driver_id, resolve_vehicle_id};
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::can_administer_transport_demand;
use business::domain::daily_schedule::DailySchedule;
use business::domain::transport_demand::TransportDemand;
use business::domain::user::User;
use business::gateway::daily_schedule_gateway::DailyScheduleGateway;
use business::gateway::user_gateway::UserGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::use_cases::daily_schedule_use_case::{
    DUPLICATE_SCHEDULE_ENTRY, DailyScheduleUseCase, NOT_A_DRIVER, NOT_A_TENANT_VEHICLE,
};
use business::use_cases::user_use_case::UserUseCase;
use business::use_cases::vehicle_use_case::VehicleUseCase;

fn use_case(state: &AppState) -> DailyScheduleUseCase {
    let db = state.conn.as_ref().clone();
    DailyScheduleUseCase::new(
        DailyScheduleGateway::new(db.clone()),
        UserGateway::new(db.clone()),
        VehicleGateway::new(db),
    )
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        NOT_A_DRIVER => ExceptionResponse::BadRequest(locale, ErrorKey::NotADriver),
        DUPLICATE_SCHEDULE_ENTRY => ExceptionResponse::Conflict(locale, ErrorKey::DuplicateScheduleEntry),
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

async fn json(state: &AppState, entry: DailySchedule) -> DailyScheduleJson {
    let driver_uuid = UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()))
        .find_by_id(entry.driver_id)
        .await
        .ok()
        .and_then(|u| u.uuid)
        .unwrap_or_default();
    let vehicle_uuid = VehicleUseCase::new(VehicleGateway::new(state.conn.as_ref().clone()))
        .find_by_id(entry.vehicle_id)
        .await
        .ok()
        .and_then(|v| v.uuid)
        .unwrap_or_default();

    DailyScheduleJson {
        uuid: entry.uuid,
        driver_uuid,
        vehicle_uuid,
        date: entry.date,
        start_time: entry.start_time,
        end_time: entry.end_time,
        notes: entry.notes,
    }
}

#[utoipa::path(
    post,
    tag = "Transport Demand",
    path = "/transport-demand/uuid/{uuid}/daily-schedule",
    params(("uuid" = String, Path, description = "Transport demand UUID")),
    request_body = DailyScheduleJson,
    responses(
        (status = 201, description = "The day's manual entry is recorded (HRMS-605). **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only).", body = DailyScheduleJson),
        (status = 400, description = "Bad request, including `NotADriver` or a vehicle outside the demand's own tenant", body = BadRequestErrorJson),
        (status = 404, description = "Demand not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller may read this demand but not adjust its schedule", body = ForbiddenErrorJson),
        (status = 409, description = "`DuplicateScheduleEntry`: this demand already has a manual entry for that date", body = BadRequestErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<DailyScheduleJson>,
) -> HttpResponse<(StatusCode, Json<DailyScheduleJson>)> {
    let demand = find_visible(&state, &locale, &current_user, uuid).await?;
    require_administrator(&locale, &current_user, &demand)?;

    let driver_id = resolve_driver_id(&state, &locale, Some(payload.driver_uuid.clone()))
        .await?
        .ok_or_else(|| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::NotADriver))?;
    let vehicle_id = resolve_vehicle_id(&state, &locale, Some(payload.vehicle_uuid.clone()))
        .await?
        .ok_or_else(|| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue))?;

    let domain = DailySchedule {
        id: None,
        uuid: None,
        tenant_id: demand.tenant_id,
        demand_id: demand.id.unwrap_or_default(),
        driver_id,
        vehicle_id,
        date: payload.date,
        start_time: payload.start_time,
        end_time: payload.end_time,
        notes: payload.notes,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };

    match use_case(&state).create(domain).await {
        Ok(entry) => Ok((StatusCode::CREATED, Json(json(&state, entry).await))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "Transport Demand",
    path = "/transport-demand/uuid/{uuid}/daily-schedules",
    params(("uuid" = String, Path, description = "Transport demand UUID"), PageQuery),
    responses(
        (status = 200, description = "The demand's manual day entries, newest first (PD-028). **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = PageJson<DailyScheduleJson>),
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
) -> HttpResponse<Json<PageJson<DailyScheduleJson>>> {
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
    path = "/transport-demand/uuid/{uuid}/daily-schedule/uuid/{entry_uuid}",
    params(
        ("uuid" = String, Path, description = "Transport demand UUID"),
        ("entry_uuid" = String, Path, description = "Schedule entry UUID"),
    ),
    responses(
        (status = 204, description = "The day's manual entry is removed. **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only)."),
        (status = 404, description = "`TransportDemandNotFound` (also for another tenant's demand, PD-034), or the entry does not belong to this demand", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller may read this demand but not adjust its schedule", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn remove(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path((uuid, entry_uuid)): Path<(String, String)>,
) -> HttpResponse<StatusCode> {
    let demand = find_visible(&state, &locale, &current_user, uuid).await?;
    require_administrator(&locale, &current_user, &demand)?;

    match use_case(&state)
        .remove(demand.id.unwrap_or_default(), entry_uuid)
        .await
    {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(_) => Err(ExceptionResponse::NotFound(
            locale,
            ErrorKey::TransportDemandNotFound,
        )),
    }
}
