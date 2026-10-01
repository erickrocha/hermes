use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson, UnauthorizedErrorJson,
};
use crate::endpoints::json::vehicle_presence_json::{VehiclePresenceEventJson, VehiclePresenceJson};
use crate::endpoints::vehicle_endpoint::find_visible;
use axum::Json;
use axum::extract::{Extension, Query, State};
use business::domain::authorization::can_publish_presence;
use business::domain::enums::{PresenceEventKind, PresenceSource};
use business::domain::user::User;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::gateway::vehicle_presence_event_gateway::VehiclePresenceEventGateway;
use business::use_cases::vehicle_presence_use_case::{
    ESTIMATE_CANNOT_STAMP, IN_THE_FUTURE, OUT_OF_ORDER, PresenceSnapshot, VehiclePresenceUseCase,
};
use std::str::FromStr;

fn use_case(state: &AppState) -> VehiclePresenceUseCase {
    let db = state.conn.as_ref().clone();
    VehiclePresenceUseCase::new(VehiclePresenceEventGateway::new(db.clone()), VehicleGateway::new(db))
}

fn body(recorded: bool, s: PresenceSnapshot) -> VehiclePresenceJson {
    VehiclePresenceJson {
        recorded,
        last_arrival_at: s.last_arrival_at,
        last_departure_at: s.last_departure_at,
        away: s.away,
    }
}

#[utoipa::path(
    post,
    tag = "VehiclePresence",
    path = "/vehicle-presence",
    request_body = VehiclePresenceEventJson,
    responses(
        (status = 200, description = "The physical arrival or departure is recorded (TRM-770…773) when it alternates with the vehicle's latest event; a departure with no arrival since, or an arrival no departure separated, changes nothing and `recorded` is false. A recorded **arrival** also opens the vehicle's triage by itself (TRM-414) -- carrying its services forward unless it returned from a trip (TRM-416/417) -- unless it already has one or the catalogue is empty. Only a real tracker reading stamps (D-24(e)); events may not go backwards or into the future. **Roles:** unbound SysAdmin only -- the stamps are the presence integration's exclusive output (TRM-788).", body = VehiclePresenceJson),
        (status = 400, description = "An unknown event or source, a schedule estimate (`D-24(e)`), or a timestamp in the future", body = BadRequestErrorJson),
        (status = 409, description = "`PresenceOutOfOrder`: earlier than the vehicle's latest event", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Only the platform's presence integration may stamp a vehicle", body = ForbiddenErrorJson),
        (status = 404, description = "Vehicle not found", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn record(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<VehiclePresenceEventJson>,
) -> HttpResponse<Json<VehiclePresenceJson>> {
    if !can_publish_presence(&current_user) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::PresenceForbidden));
    }
    let kind = PresenceEventKind::from_str(&payload.event)
        .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue))?;
    let source = match payload.source.as_deref() {
        None => PresenceSource::Tracker,
        Some(s) => PresenceSource::from_str(s)
            .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue))?,
    };
    let vehicle = find_visible(&state, &locale, &current_user, payload.vehicle_uuid).await?;
    let at = payload.occurred_at.unwrap_or_else(|| chrono::Utc::now().naive_utc());
    match use_case(&state).record(vehicle.tenant_id, vehicle.id.unwrap_or_default(), kind, at, source).await {
        Ok((recorded, snapshot)) => {
            if recorded && kind == PresenceEventKind::Arrival {
                open_triage_on_arrival(&state, vehicle.tenant_id, vehicle.id.unwrap_or_default(), &snapshot, at).await;
            }
            Ok(Json(body(recorded, snapshot)))
        }
        Err(e) if e.message == OUT_OF_ORDER => Err(ExceptionResponse::Conflict(locale, ErrorKey::PresenceOutOfOrder)),
        Err(e) if e.message == ESTIMATE_CANNOT_STAMP || e.message == IN_THE_FUTURE => {
            Err(ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue))
        }
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}

/// `TRM-414`: a recorded arrival opens the vehicle's triage. The stamp is already
/// written and stands whatever happens here -- a failure is logged, never returned.
async fn open_triage_on_arrival(state: &AppState, tenant_id: Option<i64>, vehicle_id: i64, snapshot: &PresenceSnapshot, arrived_at: chrono::NaiveDateTime) {
    use business::gateway::*;
    let db = state.conn.as_ref().clone();
    let minutes = match tenant_rule_setting_gateway::TenantRuleSettingGateway::new(db.clone()).settings_for(tenant_id).await {
        Ok(rules) => rules.garage_min_trip_absence_minutes,
        Err(_) => business::domain::tenant_rule_setting::RuleSettings::default().garage_min_trip_absence_minutes,
    };
    let triage = business::use_cases::garage_attendance_use_case::GarageAttendanceUseCase::new(
        garage_attendance_gateway::GarageAttendanceGateway::new(db.clone()),
        garage_service_gateway::GarageServiceGateway::new(db.clone()),
        garage_service_log_gateway::GarageServiceLogGateway::new(db.clone()),
        garage_service_model_gateway::GarageServiceModelGateway::new(db.clone()),
        VehicleGateway::new(db),
    );
    let run = snapshot.last_departure_at.map(|departed| (departed, arrived_at));
    if let Err(e) = triage.open_on_arrival(tenant_id, vehicle_id, run, minutes).await {
        log::error!("[VehiclePresence] the arrival's triage was not opened: {}", e.message);
    }
    // `TRM-1522`: the triage may be born hours after the fuelling that already satisfies it.
    if let Err(e) = crate::endpoints::garage_attendance_endpoint::fuelling_mark_use_case(state).apply(vehicle_id).await {
        log::error!("[VehiclePresence] the automatic fuelling mark failed: {}", e.message);
    }
}

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct PresenceQuery {
    pub vehicle_uuid: String,
}

#[utoipa::path(
    get,
    tag = "VehiclePresence",
    path = "/vehicle-presence",
    params(PresenceQuery),
    responses(
        (status = 200, description = "The vehicle's last physical arrival and departure and whether it is away (TRM-788). **Roles:** any authenticated role; own tenant only.", body = VehiclePresenceJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 404, description = "Vehicle not found, or in another tenant", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn snapshot(
    state: State<AppState>,
    Query(q): Query<PresenceQuery>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<VehiclePresenceJson>> {
    let vehicle = find_visible(&state, &locale, &current_user, q.vehicle_uuid).await?;
    match use_case(&state).snapshot(vehicle.id.unwrap_or_default()).await {
        Ok(s) => Ok(Json(body(false, s))),
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}
