use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson, UnauthorizedErrorJson,
};
use crate::endpoints::json::garage_attendance_json::{
    GarageAttendanceJson, GarageAttendanceOpenJson, GarageCallListEntryJson, GarageMatrixCellJson, GarageMatrixColumnJson, GarageMatrixJson, GarageMatrixRowJson, GarageMonitorCardJson, GarageMonitorCardsJson, GarageMonitorEntryJson, GarageQueueEntryJson, GarageServiceJson, GarageServiceMarkJson,
};
use crate::endpoints::vehicle_endpoint::find_visible;
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::commons::functions::bytes_para_string;
use business::commons::gateway::Gateway;
use business::domain::authorization::{can_operate_garage, can_read_garage};
use business::domain::enums::{GarageAttendanceOrigin, GarageServiceState};
use business::domain::garage_attendance::GarageAttendance;
use business::domain::garage_service::GarageService;
use business::domain::user::User;
use business::gateway::garage_attendance_gateway::GarageAttendanceGateway;
use business::gateway::garage_service_gateway::GarageServiceGateway;
use business::gateway::garage_service_log_gateway::GarageServiceLogGateway;
use business::gateway::garage_service_model_gateway::GarageServiceModelGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::use_cases::garage_validity::ValidityReason;
use business::use_cases::garage_readiness::required_pending;
use business::use_cases::garage_validity_use_case::{Evaluation, GarageValidityUseCase};
use business::domain::enums::GarageServiceState as ServiceState;
use business::use_cases::garage_attendance_use_case::{
    ALREADY_ACTIVE, ATTENDANCE_CLOSED, ATTENDANCE_NOT_FOUND, GarageAttendanceUseCase, NO_SERVICES, SERVICE_NOT_FOUND,
};
use std::str::FromStr;

fn use_case(state: &AppState) -> GarageAttendanceUseCase {
    let db = state.conn.as_ref().clone();
    GarageAttendanceUseCase::new(
        GarageAttendanceGateway::new(db.clone()),
        GarageServiceGateway::new(db.clone()),
        GarageServiceLogGateway::new(db.clone()),
        GarageServiceModelGateway::new(db.clone()),
        VehicleGateway::new(db),
    )
}

/// `EPIC-GA-06-S02`: the automatic fuelling mark, for the endpoints that bring a
/// fuelling or a triage into being.
pub fn fuelling_mark_use_case(state: &AppState) -> business::use_cases::garage_fuelling_mark_use_case::GarageFuellingMarkUseCase {
    use business::gateway::*;
    let db = state.conn.as_ref().clone();
    business::use_cases::garage_fuelling_mark_use_case::GarageFuellingMarkUseCase::new(
        GarageAttendanceGateway::new(db.clone()),
        GarageServiceGateway::new(db.clone()),
        GarageServiceLogGateway::new(db.clone()),
        GarageServiceModelGateway::new(db.clone()),
        fuel_entry_gateway::FuelEntryGateway::new(db.clone()),
        vehicle_presence_event_gateway::VehiclePresenceEventGateway::new(db.clone()),
        tenant_rule_setting_gateway::TenantRuleSettingGateway::new(db),
    )
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        ALREADY_ACTIVE => ExceptionResponse::Conflict(locale, ErrorKey::GarageAttendanceAlreadyActive),
        ATTENDANCE_CLOSED => ExceptionResponse::Conflict(locale, ErrorKey::GarageAttendanceClosed),
        ATTENDANCE_NOT_FOUND | SERVICE_NOT_FOUND => ExceptionResponse::NotFound(locale, ErrorKey::GarageAttendanceNotFound),
        NO_SERVICES => ExceptionResponse::BadRequest(locale, ErrorKey::GarageNoServices),
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

fn validity_use_case(state: &AppState) -> GarageValidityUseCase {
    use business::gateway::*;
    let db = state.conn.as_ref().clone();
    GarageValidityUseCase::new(
        vehicle_presence_event_gateway::VehiclePresenceEventGateway::new(db.clone()),
        extra_trip_gateway::ExtraTripGateway::new(db.clone()),
        GarageServiceModelGateway::new(db.clone()),
        tenant_rule_setting_gateway::TenantRuleSettingGateway::new(db.clone()),
        business::use_cases::fuel_gauge_use_case::FuelGaugeUseCase::new(
            fuel_entry_gateway::FuelEntryGateway::new(db.clone()),
            VehicleGateway::new(db.clone()),
            km_evolution_gateway::KmEvolutionGateway::new(db.clone()),
            fuel_gauge_setting_gateway::FuelGaugeSettingGateway::new(db.clone()),
            tenant_rule_setting_gateway::TenantRuleSettingGateway::new(db),
        ),
    )
}

/// The evaluation of a triage's services, falling back to the stored states if
/// the evaluation itself fails -- a screen never loses the record.
async fn evaluations(state: &AppState, attendance: &GarageAttendance, services: &[GarageService]) -> Evaluation {
    validity_use_case(state).evaluate_full(attendance, services).await.unwrap_or_else(|_| Evaluation {
        services: services.iter().map(|s| (s.state, ValidityReason::Valid)).collect(),
        required: vec![false; services.len()],
        names: vec![String::new(); services.len()],
        tank: business::use_cases::garage_validity::TankFact::NoUsableReading,
        rules: Default::default(),
    })
}

async fn service_json(state: &AppState, s: GarageService, effective: (ServiceState, ValidityReason)) -> GarageServiceJson {
    let model = GarageServiceModelGateway::new(state.conn.as_ref().clone())
        .find_by_id(s.service_model_id)
        .await
        .ok()
        .flatten();
    GarageServiceJson {
        uuid: s.uuid,
        service_model_uuid: model.as_ref().map(|m| bytes_para_string(m.uuid.clone())),
        name: model.as_ref().map(|m| m.name.clone()).unwrap_or(s.name_key),
        service_group: model.as_ref().map(|m| m.service_group.clone()).unwrap_or_default(),
        required_for_departure: model.as_ref().is_some_and(|m| m.required_for_departure),
        state: s.state.to_string(),
        effective_state: effective.0.to_string(),
        effective_reason: format!("{:?}", effective.1),
        performed_at: s.performed_at,
        marked_at: s.marked_at,
        forced_pending_at: s.forced_pending_at,
    }
}

async fn json(state: &AppState, a: GarageAttendance, services: Vec<GarageService>) -> GarageAttendanceJson {
    let vehicle_uuid = VehicleGateway::new(state.conn.as_ref().clone())
        .find_by_id(a.vehicle_id)
        .await
        .ok()
        .flatten()
        .map(|m| bytes_para_string(m.uuid))
        .unwrap_or_default();
    let evaluation = evaluations(state, &a, &services).await;
    let pairs: Vec<_> = evaluation.required.iter().copied().zip(evaluation.services.iter().map(|(s, _)| *s)).collect();
    let blocking: Vec<String> = required_pending(&pairs).into_iter().map(|i| evaluation.names[i].clone()).collect();
    let mut rows = Vec::with_capacity(services.len());
    for (s, e) in services.into_iter().zip(evaluation.services) {
        rows.push(service_json(state, s, e).await);
    }
    GarageAttendanceJson {
        uuid: a.uuid,
        vehicle_uuid,
        attendance_date: a.attendance_date,
        checked_in_at: a.checked_in_at,
        status: a.status.to_string(),
        manual_priority: a.manual_priority,
        released_at: a.released_at,
        origin: a.origin.to_string(),
        required_pending: blocking,
        services: rows,
    }
}

/// Finds a triage the caller may see, and (when `operate`) may act on.
async fn visible(
    state: &AppState,
    locale: &Locale,
    current_user: &User,
    uuid: &str,
    operate: bool,
) -> Result<(GarageAttendance, Vec<GarageService>), ExceptionResponse> {
    let (attendance, services) = use_case(state)
        .find_by_uuid(uuid.to_string())
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::GarageAttendanceNotFound))?;
    if !can_read_garage(current_user, attendance.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale.clone(), ErrorKey::GarageAttendanceNotFound));
    }
    if operate && !can_operate_garage(current_user, attendance.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale.clone(), ErrorKey::GarageForbidden));
    }
    Ok((attendance, services))
}

#[utoipa::path(
    post,
    tag = "GarageAttendance",
    path = "/garage-attendance",
    request_body = GarageAttendanceOpenJson,
    responses(
        (status = 201, description = "A triage is opened for the vehicle (TRM-410) with one pending service record per active catalogue entry. **Roles:** SysAdmin (unbound), TenantOwner, Mechanic (own tenant only).", body = GarageAttendanceJson),
        (status = 400, description = "`GarageNoServices`: the catalogue has no active service", body = BadRequestErrorJson),
        (status = 409, description = "`GarageAttendanceAlreadyActive`: the vehicle already has an active triage (TRM-412)", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not run the garage", body = ForbiddenErrorJson),
        (status = 404, description = "Vehicle not found, or in another tenant", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn open(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<GarageAttendanceOpenJson>,
) -> HttpResponse<(StatusCode, Json<GarageAttendanceJson>)> {
    let vehicle = find_visible(&state, &locale, &current_user, payload.vehicle_uuid).await?;
    if !can_operate_garage(&current_user, vehicle.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::GarageForbidden));
    }
    match use_case(&state)
        .open(vehicle.tenant_id, vehicle.id.unwrap_or_default(), payload.manual_priority, GarageAttendanceOrigin::Manual)
        .await
    {
        Ok((attendance, services)) => Ok((StatusCode::CREATED, Json(json(&state, attendance, services).await))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "GarageAttendance",
    path = "/garage-attendance/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Triage UUID")),
    responses(
        (status = 200, description = "The triage and its service records. **Roles:** any caller of the triage's own tenant; SysAdmin.", body = GarageAttendanceJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 404, description = "Triage not found, **or in another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_by_uuid(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<GarageAttendanceJson>> {
    let (attendance, services) = visible(&state, &locale, &current_user, &uuid, false).await?;
    Ok(Json(json(&state, attendance, services).await))
}

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ActiveQuery {
    pub vehicle_uuid: String,
}

#[utoipa::path(
    get,
    tag = "GarageAttendance",
    path = "/garage-attendance/active",
    params(ActiveQuery),
    responses(
        (status = 200, description = "The vehicle's one active triage (TRM-412) with its service records. **Roles:** any authenticated role; own tenant only.", body = GarageAttendanceJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 404, description = "The vehicle was not found, or has no active triage", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn active(
    state: State<AppState>,
    Query(q): Query<ActiveQuery>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<GarageAttendanceJson>> {
    let vehicle = find_visible(&state, &locale, &current_user, q.vehicle_uuid).await?;
    match use_case(&state).find_active(vehicle.id.unwrap_or_default()).await {
        Ok(Some((attendance, services))) => Ok(Json(json(&state, attendance, services).await)),
        Ok(None) => Err(ExceptionResponse::NotFound(locale, ErrorKey::GarageAttendanceNotFound)),
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}

#[utoipa::path(
    put,
    tag = "GarageAttendance",
    path = "/garage-attendance/uuid/{uuid}/service/{service_uuid}",
    params(
        ("uuid" = String, Path, description = "Triage UUID"),
        ("service_uuid" = String, Path, description = "Service record UUID"),
    ),
    request_body = GarageServiceMarkJson,
    responses(
        (status = 200, description = "The service is marked (TRM-437/438): `Performed` stamps the performance instant, `NotNeeded`/`NotDone` the marking instant, `Pending` the forcing instant, and an independent audit row is written with it (TRM-440). Only an open triage can be marked. **Roles:** SysAdmin (unbound), TenantOwner, Mechanic (own tenant only).", body = GarageServiceJson),
        (status = 400, description = "A state other than Pending/Performed/NotNeeded/NotDone", body = BadRequestErrorJson),
        (status = 409, description = "`GarageAttendanceClosed`: the triage is already closed", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not run the garage", body = ForbiddenErrorJson),
        (status = 404, description = "Triage or service not found, **or in another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn mark_service(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path((uuid, service_uuid)): Path<(String, String)>,
    Json(payload): Json<GarageServiceMarkJson>,
) -> HttpResponse<Json<GarageServiceJson>> {
    let (attendance, _) = visible(&state, &locale, &current_user, &uuid, true).await?;
    let new_state = GarageServiceState::from_str(&payload.state)
        .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue))?;
    match use_case(&state).mark_service(uuid, service_uuid, new_state, current_user.id).await {
        Ok(service) => {
            let effective = evaluations(&state, &attendance, std::slice::from_ref(&service)).await.services.remove(0);
            Ok(Json(service_json(&state, service, effective).await))
        }
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    post,
    tag = "GarageAttendance",
    path = "/garage-attendance/uuid/{uuid}/checkout",
    params(("uuid" = String, Path, description = "Triage UUID")),
    responses(
        (status = 200, description = "An administrative check-out (TRM-423): the triage is closed as `Finished`, the release time recorded, the active slot freed and the manual priority zeroed (TRM-425). **Roles:** SysAdmin (unbound), TenantOwner, Mechanic (own tenant only).", body = GarageAttendanceJson),
        (status = 409, description = "`GarageAttendanceClosed`: the triage is already closed", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not run the garage", body = ForbiddenErrorJson),
        (status = 404, description = "Triage not found, **or in another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn checkout(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<GarageAttendanceJson>> {
    let (_, services) = visible(&state, &locale, &current_user, &uuid, true).await?;
    match use_case(&state).checkout(uuid).await {
        Ok(attendance) => Ok(Json(json(&state, attendance, services).await)),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "GarageAttendance",
    path = "/garage-attendance/queue",
    responses(
        (status = 200, description = "The yard's queue: every active triage of the caller's tenant in the one order every surface shows (TRM-480/482) -- manual priority first (lower is more urgent), then today's trip departures by time, then the later days of the horizon (tomorrow; on Friday the whole weekend, TRM-481), then vehicles with none, by prefix. Each entry lists the required services still pending (TRM-469…471) and the fill-the-tank alert (TRM-474), which is visual only and never reorders. Departures come from registered trips only. **Roles:** any authenticated role; own tenant only.", body = Vec<GarageQueueEntryJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn queue(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(_current_user): Extension<User>,
) -> HttpResponse<Json<Vec<GarageQueueEntryJson>>> {
    match queue_use_case(&state).queue().await {
        Ok(entries) => Ok(Json(entries.iter().map(queue_json).collect())),
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}

fn queue_use_case(state: &AppState) -> business::use_cases::garage_queue_use_case::GarageQueueUseCase {
    use business::gateway::*;
    use business::use_cases::garage_call_use_case::GarageCallUseCase;
    let db = state.conn.as_ref().clone();
    business::use_cases::garage_queue_use_case::GarageQueueUseCase::new(
        GarageAttendanceGateway::new(db.clone()),
        GarageServiceGateway::new(db.clone()),
        crate::endpoints::schedule_endpoint::schedule_use_case(state),
        VehicleGateway::new(db.clone()),
        validity_use_case(state),
        GarageCallUseCase::new(
            garage_call_gateway::GarageCallGateway::new(db.clone()),
            vehicle_presence_event_gateway::VehiclePresenceEventGateway::new(db.clone()),
            VehicleGateway::new(db.clone()),
        ),
        GarageServiceModelGateway::new(db),
    )
}

fn queue_json(e: &business::use_cases::garage_queue_use_case::QueueEntry) -> GarageQueueEntryJson {
    GarageQueueEntryJson {
        attendance_uuid: e.attendance.uuid.clone(),
        vehicle_uuid: e.vehicle_uuid.clone(),
        prefix: e.prefix.clone(),
        manual_priority: e.attendance.manual_priority,
        next_departure: e.next_departure,
        required_pending: e.required_pending.clone(),
        fill_tank_alert: e.fill_tank_alert,
        called_by_manager: e.called_by_manager,
    }
}

#[utoipa::path(
    get,
    tag = "GarageAttendance",
    path = "/garage-attendance/monitor",
    responses(
        (status = 200, description = "The wall monitor's queue (TRM-497): the yard's vehicles with imminent work outstanding first, vehicles with nothing left to do last to free the display, then manual priority, next departure and prefix. Each entry carries the departure's readiness (TRM-499: `Alert` when a required service is pending inside the window of a trip (default 120 min) or a line (30 min); `Preparing` when anything is outstanding; `Ready` otherwise; absent with no departure) and `urgent` (TRM-498: an extra trip inside its window (90 min) with a required service pending -- never a line). The windows are per-tenant settings. Read-only: it writes nothing (TRM-493). **Roles:** any authenticated role; own tenant only.", body = Vec<GarageMonitorEntryJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn monitor(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(_current_user): Extension<User>,
) -> HttpResponse<Json<Vec<GarageMonitorEntryJson>>> {
    use business::use_cases::garage_readiness::{departure_readiness, urgent_animation};
    let now = chrono::Utc::now().naive_utc();
    match queue_use_case(&state).monitor().await {
        Ok(entries) => Ok(Json(
            entries
                .iter()
                .map(|e| {
                    let pending = !e.required_pending.is_empty();
                    GarageMonitorEntryJson {
                        entry: queue_json(e),
                        readiness: e.next_departure.map(|at| {
                            format!("{:?}", departure_readiness(at, e.next_is_trip, now, pending, e.outstanding, &e.rules))
                        }),
                        urgent: e.next_departure.is_some_and(|at| urgent_animation(at, e.next_is_trip, now, pending, &e.rules)),
                    }
                })
                .collect(),
        )),
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}

#[utoipa::path(
    get,
    tag = "GarageAttendance",
    path = "/garage-attendance/monitor/cards",
    responses(
        (status = 200, description = "The wall monitor's vehicle cards (TRM-495/1512): vehicles in the garage, need-to-refuel first (the same fill-the-tank alert as every other surface) and then the lowest tank; a vehicle with no trustworthy reading after those with one. Capped at the tenant's card limit (default 6) with `notShown` saying how many were left out. Read-only. The maintenance situation and the last garage-cycle distance the legacy card also carried are not yet included. **Roles:** any authenticated role; own tenant only.", body = GarageMonitorCardsJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn monitor_cards(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(_current_user): Extension<User>,
) -> HttpResponse<Json<GarageMonitorCardsJson>> {
    match queue_use_case(&state).cards().await {
        Ok((cards, not_shown)) => Ok(Json(GarageMonitorCardsJson {
            cards: cards
                .into_iter()
                .map(|c| GarageMonitorCardJson {
                    vehicle_uuid: c.vehicle_uuid,
                    prefix: c.prefix,
                    tank_percent: c.tank_percent,
                    needs_refuel: c.needs_refuel,
                })
                .collect(),
            not_shown,
        })),
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}

#[utoipa::path(
    get,
    tag = "GarageAttendance",
    path = "/garage-attendance/call-list",
    responses(
        (status = 200, description = "The call-to-base list (TRM-485/487/1504): vehicles away from base that have a manual call in force (listed even with no trip scheduled and every service in order), a further departure still ahead (a started one is ignored), or a tank that calls on its own (the alert cuts, never a second computation). Manual calls first, then the next departure, then prefix. Read-only: it never changes a vehicle's tag -- only the tracker confirms arrival. Known gap: a resolved fuelling service does not yet suppress a tank call (TRM-477/1508). **Roles:** any authenticated role; own tenant only.", body = Vec<GarageCallListEntryJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn call_list(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(_current_user): Extension<User>,
) -> HttpResponse<Json<Vec<GarageCallListEntryJson>>> {
    match queue_use_case(&state).call_list().await {
        Ok(entries) => Ok(Json(
            entries
                .into_iter()
                .map(|e| GarageCallListEntryJson {
                    vehicle_uuid: e.vehicle_uuid,
                    prefix: e.prefix,
                    next_departure: e.next_departure,
                    called_by_manager: e.reasons.manual,
                    has_departure_ahead: e.reasons.departure,
                    tank_calls: e.reasons.tank,
                })
                .collect(),
        )),
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}

#[utoipa::path(
    get,
    tag = "GarageAttendance",
    path = "/garage-attendance/monitor/matrix",
    responses(
        (status = 200, description = "The wall monitor's services matrix (TRM-496): active triages (rows, in the monitor's order) against the catalogue's active services (columns, external before internal). Each cell carries the service's effective state, the stamp of its stored state (performance, marking or forcing instant) and, for a pending cell, the last time that service was performed on that vehicle. A cell is absent where the triage has no record of the service. Capped at the tenant's matrix size (default 10 rows x 8 columns) with the counts left out. Read-only. **Roles:** any authenticated role; own tenant only.", body = GarageMatrixJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn monitor_matrix(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(_current_user): Extension<User>,
) -> HttpResponse<Json<GarageMatrixJson>> {
    match queue_use_case(&state).matrix().await {
        Ok(m) => Ok(Json(GarageMatrixJson {
            columns: m
                .columns
                .into_iter()
                .map(|c| GarageMatrixColumnJson { service_model_uuid: c.uuid, name: c.name, internal: c.internal })
                .collect(),
            rows: m
                .rows
                .into_iter()
                .map(|r| GarageMatrixRowJson {
                    vehicle_uuid: r.vehicle_uuid,
                    prefix: r.prefix,
                    cells: r
                        .cells
                        .into_iter()
                        .map(|c| {
                            c.map(|c| GarageMatrixCellJson {
                                effective_state: c.effective_state.to_string(),
                                stamp: c.stamp,
                                last_performed_at: c.last_performed_at,
                            })
                        })
                        .collect(),
                })
                .collect(),
            rows_not_shown: m.rows_not_shown,
            columns_not_shown: m.columns_not_shown,
        })),
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}
