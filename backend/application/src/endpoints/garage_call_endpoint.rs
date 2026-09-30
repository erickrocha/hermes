use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson, UnauthorizedErrorJson,
};
use crate::endpoints::json::garage_attendance_json::{GarageCallJson, GarageCallRequestJson};
use crate::endpoints::vehicle_endpoint::find_visible;
use axum::Json;
use axum::extract::{Extension, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::can_call_vehicle_to_base;
use business::domain::garage_call::GarageCall;
use business::domain::user::User;
use business::gateway::garage_call_gateway::GarageCallGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::gateway::vehicle_presence_event_gateway::VehiclePresenceEventGateway;
use business::use_cases::garage_call_use_case::{GarageCallUseCase, NOT_AWAY, NO_ACTIVE_CALL};

fn use_case(state: &AppState) -> GarageCallUseCase {
    let db = state.conn.as_ref().clone();
    GarageCallUseCase::new(
        GarageCallGateway::new(db.clone()),
        VehiclePresenceEventGateway::new(db.clone()),
        VehicleGateway::new(db),
    )
}

fn body(call: Option<GarageCall>) -> GarageCallJson {
    GarageCallJson { active: call.is_some(), called_at: call.map(|c| c.called_at) }
}

#[utoipa::path(
    post,
    tag = "GarageCall",
    path = "/garage-call",
    request_body = GarageCallRequestJson,
    responses(
        (status = 201, description = "The vehicle is called to base, with a dedicated stamp (TRM-483). It stays in force until the vehicle's physical arrival is later than the call -- never cleared by its tag, nobody needs to unmark it (TRM-484) -- and outranks the computed readiness (TRM-485). **Roles:** SysAdmin (unbound), TenantOwner (own tenant only).", body = GarageCallJson),
        (status = 200, description = "A call was already in force for this vehicle; nothing new was stamped", body = GarageCallJson),
        (status = 400, description = "`GarageVehicleNotAway`: only a vehicle away from base can be called (TRM-486)", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Only the manager may call a vehicle to base", body = ForbiddenErrorJson),
        (status = 404, description = "Vehicle not found, or in another tenant", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn call(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<GarageCallRequestJson>,
) -> HttpResponse<(StatusCode, Json<GarageCallJson>)> {
    let vehicle = find_visible(&state, &locale, &current_user, payload.vehicle_uuid).await?;
    if !can_call_vehicle_to_base(&current_user, vehicle.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::GarageForbidden));
    }
    match use_case(&state).call(vehicle.tenant_id, vehicle.id.unwrap_or_default(), current_user.id).await {
        Ok((call, created)) => {
            let status = if created { StatusCode::CREATED } else { StatusCode::OK };
            Ok((status, Json(body(Some(call)))))
        }
        Err(e) if e.message == NOT_AWAY => Err(ExceptionResponse::BadRequest(locale, ErrorKey::GarageVehicleNotAway)),
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}

#[utoipa::path(
    post,
    tag = "GarageCall",
    path = "/garage-call/cancel",
    request_body = GarageCallRequestJson,
    responses(
        (status = 200, description = "The call in force is cancelled -- for a call made in error (TRM-486). **Roles:** SysAdmin (unbound), TenantOwner (own tenant only).", body = GarageCallJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Only the manager may cancel a call", body = ForbiddenErrorJson),
        (status = 404, description = "Vehicle not found, or it has no call in force", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn cancel(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<GarageCallRequestJson>,
) -> HttpResponse<Json<GarageCallJson>> {
    let vehicle = find_visible(&state, &locale, &current_user, payload.vehicle_uuid).await?;
    if !can_call_vehicle_to_base(&current_user, vehicle.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::GarageForbidden));
    }
    match use_case(&state).cancel(vehicle.id.unwrap_or_default()).await {
        Ok(_) => Ok(Json(body(None))),
        Err(e) if e.message == NO_ACTIVE_CALL => Err(ExceptionResponse::NotFound(locale, ErrorKey::GarageAttendanceNotFound)),
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct CallQuery {
    pub vehicle_uuid: String,
}

#[utoipa::path(
    get,
    tag = "GarageCall",
    path = "/garage-call",
    params(CallQuery),
    responses(
        (status = 200, description = "Whether the manager's call is in force for the vehicle (TRM-484). **Roles:** any authenticated role; own tenant only.", body = GarageCallJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 404, description = "Vehicle not found, or in another tenant", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_active(
    state: State<AppState>,
    Query(q): Query<CallQuery>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<GarageCallJson>> {
    let vehicle = find_visible(&state, &locale, &current_user, q.vehicle_uuid).await?;
    match use_case(&state).active(vehicle.id.unwrap_or_default()).await {
        Ok(call) => Ok(Json(body(call))),
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}
