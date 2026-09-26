use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::json::vehicle_assignment_json::{AssignDriverRequest, VehicleAssignmentJson};
use crate::endpoints::vehicle_endpoint::find_visible;
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::can_administer_vehicle;
use business::domain::user::User;
use business::domain::vehicle::Vehicle;
use business::domain::vehicle_assignment::VehicleAssignment;
use business::gateway::user_gateway::UserGateway;
use business::gateway::vehicle_assignment_gateway::VehicleAssignmentGateway;
use business::use_cases::vehicle_assignment_use_case::{
    NO_LIVE_ASSIGNMENT, NOT_A_DRIVER, VEHICLE_ALREADY_ASSIGNED, VehicleAssignmentUseCase,
};

fn use_case(state: &AppState) -> VehicleAssignmentUseCase {
    let db = state.conn.as_ref().clone();
    VehicleAssignmentUseCase::new(VehicleAssignmentGateway::new(db.clone()), UserGateway::new(db))
}

fn json(vehicle: &Vehicle, (assignment, driver): (VehicleAssignment, User)) -> VehicleAssignmentJson {
    VehicleAssignmentJson {
        uuid: assignment.uuid,
        vehicle_uuid: vehicle.uuid.clone(),
        driver_uuid: driver.uuid,
        driver_name: driver.name,
        started_at: assignment.started_at,
        ended_at: assignment.ended_at,
    }
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        VEHICLE_ALREADY_ASSIGNED => ExceptionResponse::Conflict(locale, ErrorKey::VehicleAlreadyAssigned),
        NO_LIVE_ASSIGNMENT => ExceptionResponse::NotFound(locale, ErrorKey::NoLiveAssignment),
        NOT_A_DRIVER => ExceptionResponse::BadRequest(locale, ErrorKey::NotADriver),
        _ => ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError),
    }
}

/// HRMS-923's hierarchy governs assignments too: the one who administers
/// the vehicle assigns and ends. The vehicle was found first, so 403 here
/// discloses nothing the caller could not already see.
fn require_administrator(locale: &Locale, user: &User, vehicle: &Vehicle) -> Result<(), ExceptionResponse> {
    if can_administer_vehicle(user, vehicle.tenant_id) {
        Ok(())
    } else {
        Err(ExceptionResponse::Forbidden(locale.clone(), ErrorKey::VehicleForbidden))
    }
}

#[utoipa::path(
    post,
    tag = "Vehicle",
    path = "/vehicle/uuid/{uuid}/assignment",
    params(("uuid" = String, Path, description = "Vehicle UUID")),
    request_body = AssignDriverRequest,
    responses(
        (status = 201, description = "The driver now answers for the vehicle (HRMS-931). **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only).", body = VehicleAssignmentJson),
        (status = 400, description = "`NotADriver`: the person named is not an active `Driver` of the vehicle's own tenant (HRMS-932)", body = BadRequestErrorJson),
        (status = 404, description = "Vehicle not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller may read this vehicle but not assign it (HRMS-923)", body = ForbiddenErrorJson),
        (status = 409, description = "`VehicleAlreadyAssigned`: the vehicle already has a live assignment; end it first (D-23(d), HRMS-934)", body = BadRequestErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn assign(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<AssignDriverRequest>,
) -> HttpResponse<(StatusCode, Json<VehicleAssignmentJson>)> {
    let vehicle = find_visible(&state, &locale, &current_user, uuid).await?;
    require_administrator(&locale, &current_user, &vehicle)?;
    match use_case(&state).assign(&vehicle, payload.driver_uuid).await {
        Ok(result) => Ok((StatusCode::CREATED, Json(json(&vehicle, result)))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "Vehicle",
    path = "/vehicle/uuid/{uuid}/assignment",
    params(("uuid" = String, Path, description = "Vehicle UUID")),
    responses(
        (status = 200, description = "The vehicle's live assignment. **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = VehicleAssignmentJson),
        (status = 404, description = "`VehicleNotFound` (also for another tenant's vehicle, PD-034) or `NoLiveAssignment`", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn current(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<VehicleAssignmentJson>> {
    let vehicle = find_visible(&state, &locale, &current_user, uuid).await?;
    match use_case(&state).current(&vehicle).await {
        Ok(result) => Ok(Json(json(&vehicle, result))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    put,
    tag = "Vehicle",
    path = "/vehicle/uuid/{uuid}/assignment/end",
    params(("uuid" = String, Path, description = "Vehicle UUID")),
    responses(
        (status = 200, description = "The live assignment ended now; it stays in the history (HRMS-933). **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only).", body = VehicleAssignmentJson),
        (status = 404, description = "`VehicleNotFound` (also for another tenant's vehicle, PD-034) or `NoLiveAssignment`", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller may read this vehicle but not end its assignment (HRMS-923)", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn end(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<VehicleAssignmentJson>> {
    let vehicle = find_visible(&state, &locale, &current_user, uuid).await?;
    require_administrator(&locale, &current_user, &vehicle)?;
    match use_case(&state).end(&vehicle).await {
        Ok(result) => Ok(Json(json(&vehicle, result))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "Vehicle",
    path = "/vehicle/uuid/{uuid}/assignments",
    params(("uuid" = String, Path, description = "Vehicle UUID"), PageQuery),
    responses(
        (status = 200, description = "The vehicle's assignment history, newest first (PD-028). **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = PageJson<VehicleAssignmentJson>),
        (status = 404, description = "Vehicle not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
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
) -> HttpResponse<Json<PageJson<VehicleAssignmentJson>>> {
    let vehicle = find_visible(&state, &locale, &current_user, uuid).await?;
    let (page, page_size) = (page_query.page(), page_query.page_size());
    match use_case(&state).history(&vehicle, page, page_size).await {
        Ok((items, total)) => Ok(Json(PageJson::new(
            items.into_iter().map(|item| json(&vehicle, item)).collect(),
            page,
            page_size,
            total,
        ))),
        Err(e) => Err(error(locale, &e.message)),
    }
}
