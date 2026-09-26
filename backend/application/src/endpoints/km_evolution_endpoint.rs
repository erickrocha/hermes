use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::km_evolution_json::KmEvolutionJson;
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::vehicle_endpoint::find_visible;
use crate::infrastructure::mapper::reject_unknown_km_origin;
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::can_administer_vehicle;
use business::domain::enums::KmOrigin;
use business::domain::km_evolution::KmEvolution;
use business::domain::user::User;
use business::domain::vehicle::Vehicle;
use business::gateway::km_evolution_gateway::KmEvolutionGateway;
use business::gateway::user_gateway::UserGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::use_cases::km_evolution_use_case::KmEvolutionUseCase;
use business::use_cases::user_use_case::UserUseCase;
use std::str::FromStr;

fn use_case(state: &AppState) -> KmEvolutionUseCase {
    let db = state.conn.as_ref().clone();
    KmEvolutionUseCase::new(KmEvolutionGateway::new(db.clone()), VehicleGateway::new(db))
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    let _ = message;
    ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)
}

fn require_administrator(
    locale: &Locale,
    user: &User,
    vehicle: &Vehicle,
) -> Result<(), ExceptionResponse> {
    if can_administer_vehicle(user, vehicle.tenant_id) {
        Ok(())
    } else {
        Err(ExceptionResponse::Forbidden(locale.clone(), ErrorKey::VehicleForbidden))
    }
}

/// Resolves the recording user's uuid, when named -- no role restriction:
/// any authenticated user administering the vehicle may be credited with a
/// reading, unlike a driver assignment.
async fn resolve_recorded_by(
    state: &AppState,
    locale: &Locale,
    uuid: Option<String>,
) -> Result<Option<i64>, ExceptionResponse> {
    let Some(uuid) = uuid else { return Ok(None) };
    let user = UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()))
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue))?;
    Ok(user.id)
}

async fn json(state: &AppState, reading: KmEvolution) -> KmEvolutionJson {
    let recorded_by_uuid = match reading.recorded_by_user_id {
        Some(id) => UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()))
            .find_by_id(id)
            .await
            .ok()
            .and_then(|u| u.uuid),
        None => None,
    };

    KmEvolutionJson {
        uuid: reading.uuid,
        km: reading.km,
        recorded_at: reading.recorded_at,
        origin: reading.origin.to_string(),
        source_entity: reading.source_entity,
        source_entity_id: reading.source_entity_id,
        notes: reading.notes,
        recorded_by_uuid,
    }
}

#[utoipa::path(
    post,
    tag = "Vehicle",
    path = "/vehicle/uuid/{uuid}/km-evolution",
    params(("uuid" = String, Path, description = "Vehicle UUID")),
    request_body = KmEvolutionJson,
    responses(
        (status = 201, description = "The reading is recorded and `vehicle.odometer_km` is recomputed from the chronologically latest entry (HRMS-650, AD-041). **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only).", body = KmEvolutionJson),
        (status = 400, description = "Bad request, including an origin outside TRM-151's seven-value vocabulary or a negative reading", body = BadRequestErrorJson),
        (status = 404, description = "Vehicle not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller may read this vehicle but not record a reading against it", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<KmEvolutionJson>,
) -> HttpResponse<(StatusCode, Json<KmEvolutionJson>)> {
    let vehicle = find_visible(&state, &locale, &current_user, uuid).await?;
    require_administrator(&locale, &current_user, &vehicle)?;
    reject_unknown_km_origin(&payload.origin, &locale)?;

    let recorded_by_user_id =
        resolve_recorded_by(&state, &locale, payload.recorded_by_uuid.clone()).await?;

    let domain = KmEvolution {
        id: None,
        uuid: None,
        tenant_id: vehicle.tenant_id,
        vehicle_id: vehicle.id.unwrap_or_default(),
        km: payload.km,
        recorded_at: payload.recorded_at,
        origin: KmOrigin::from_str(&payload.origin).unwrap_or(KmOrigin::Manual),
        source_entity: payload.source_entity,
        source_entity_id: payload.source_entity_id,
        notes: payload.notes,
        recorded_by_user_id,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };

    match use_case(&state).create(domain).await {
        Ok(reading) => Ok((StatusCode::CREATED, Json(json(&state, reading).await))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "Vehicle",
    path = "/vehicle/uuid/{uuid}/km-evolutions",
    params(("uuid" = String, Path, description = "Vehicle UUID"), PageQuery),
    responses(
        (status = 200, description = "The vehicle's odometer history, most recently recorded first (PD-028). **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = PageJson<KmEvolutionJson>),
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
) -> HttpResponse<Json<PageJson<KmEvolutionJson>>> {
    let vehicle = find_visible(&state, &locale, &current_user, uuid).await?;
    let (page, page_size) = (page_query.page(), page_query.page_size());
    match use_case(&state)
        .history(vehicle.id.unwrap_or_default(), page, page_size)
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
