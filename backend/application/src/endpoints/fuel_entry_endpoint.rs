use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::fuel_entry_json::{FuelEntryJson, FuelReportJson, FuelSyncOutcomeJson};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::vehicle_endpoint::find_visible;
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::commons::functions::bytes_para_string;
use business::commons::gateway::Gateway;
use business::domain::authorization::{can_create_fuel_entry, can_read_fuel_entry};
use business::domain::enums::FuelEntryOrigin;
use business::domain::fuel_entry::FuelEntry;
use business::domain::user::User;
use business::gateway::fuel_entry_gateway::FuelEntryGateway;
use business::gateway::fuel_provider::configured_provider;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::use_cases::fuel_entry_use_case::{
    FuelEntryUseCase, VALUE_MUST_NOT_BE_NEGATIVE, VEHICLE_NOT_FOUND, VOLUME_MUST_BE_POSITIVE,
};
use business::use_cases::fuel_sync_use_case::FuelSyncUseCase;

fn use_case(state: &AppState) -> FuelEntryUseCase {
    let db = state.conn.as_ref().clone();
    FuelEntryUseCase::new(FuelEntryGateway::new(db.clone()), VehicleGateway::new(db))
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        VEHICLE_NOT_FOUND | VOLUME_MUST_BE_POSITIVE | VALUE_MUST_NOT_BE_NEGATIVE => {
            ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)
        }
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

async fn json(state: &AppState, entry: FuelEntry) -> FuelEntryJson {
    let vehicle_uuid = VehicleGateway::new(state.conn.as_ref().clone())
        .find_by_id(entry.vehicle_id)
        .await
        .ok()
        .flatten()
        .map(|m| bytes_para_string(m.uuid))
        .unwrap_or_default();

    FuelEntryJson {
        uuid: entry.uuid,
        tenant_id: entry.tenant_id,
        vehicle_uuid,
        recorded_at: entry.recorded_at,
        volume_liters: entry.volume_liters,
        value_cents: entry.value_cents,
        odometer_km: entry.odometer_km,
        station: entry.station,
        full_tank: entry.full_tank,
        origin: Some(entry.origin.to_string()),
    }
}

#[utoipa::path(
    post,
    tag = "FuelEntry",
    path = "/fuel-entry",
    request_body = FuelEntryJson,
    responses(
        (status = 201, description = "The fuelling is recorded (TRM-514's base fields). `origin` is server-derived (`Manual`), never taken from the caller. **Roles:** SysAdmin (unbound; names the owning tenant); TenantOwner, Mechanic (own tenant only).", body = FuelEntryJson),
        (status = 400, description = "Bad request, including a non-positive volume, a negative value, or an unknown vehicle", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not record fuel entries", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<FuelEntryJson>,
) -> HttpResponse<(StatusCode, Json<FuelEntryJson>)> {
    if !can_create_fuel_entry(&current_user, payload.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::FuelEntryForbidden));
    }

    let mut tenant_id = payload.tenant_id;
    if current_user.tenant_id.is_some() {
        tenant_id = current_user.tenant_id;
    }

    let vehicle = find_visible(&state, &locale, &current_user, payload.vehicle_uuid).await?;

    let entry = FuelEntry {
        id: None,
        uuid: None,
        tenant_id,
        vehicle_id: vehicle.id.unwrap_or_default(),
        recorded_at: payload.recorded_at,
        volume_liters: payload.volume_liters,
        value_cents: payload.value_cents,
        odometer_km: payload.odometer_km,
        station: payload.station,
        full_tank: payload.full_tank,
        origin: FuelEntryOrigin::Manual,
        provider_transaction_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };

    match use_case(&state).create(entry).await {
        Ok(saved) => Ok((StatusCode::CREATED, Json(json(&state, saved).await))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "FuelEntry",
    path = "/fuel-entry/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Fuel entry UUID")),
    responses(
        (status = 200, description = "The fuel entry. **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = FuelEntryJson),
        (status = 404, description = "Fuel entry not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_by_uuid(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<FuelEntryJson>> {
    let entry = use_case(&state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::FuelEntryNotFound))?;
    if !can_read_fuel_entry(&current_user, entry.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::FuelEntryNotFound));
    }
    Ok(Json(json(&state, entry).await))
}

#[utoipa::path(
    get,
    tag = "FuelEntry",
    path = "/fuel-entry",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of the caller's own tenant's fuel entries (PD-028), most recent first. **Roles:** SysAdmin, TenantOwner, TenantUser, Driver, Mechanic (any authenticated role; SysAdmin sees every tenant, everyone else only their own).", body = PageJson<FuelEntryJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_all(
    state: State<AppState>,
    Query(page_query): Query<PageQuery>,
    Extension(locale): Extension<Locale>,
    Extension(_current_user): Extension<User>,
) -> HttpResponse<Json<PageJson<FuelEntryJson>>> {
    let (page, page_size) = (page_query.page(), page_query.page_size());
    match use_case(&state).find_page(page, page_size).await {
        Ok((entries, total)) => {
            let mut rows = Vec::with_capacity(entries.len());
            for entry in entries {
                rows.push(json(&state, entry).await);
            }
            Ok(Json(PageJson::new(rows, page, page_size, total)))
        }
        Err(_) => Err(ExceptionResponse::InternalServerError(
            locale,
            ErrorKey::UnexpectedError,
        )),
    }
}

#[utoipa::path(
    post,
    tag = "FuelEntry",
    path = "/fuel-entry/sync",
    responses(
        (status = 200, description = "Pulls the oldest pending batch from the fuel-management provider (TRM-500…517), matches each transaction to a vehicle by fleet prefix then plate (TRM-507), imports a new one or corrects an existing one in place by the provider's own transaction id (TRM-511/512), and acknowledges the batch. Runs against the caller's own tenant's vehicles. **Roles:** TenantOwner, Mechanic (own tenant only) -- an unbound SysAdmin has no single tenant's fleet to sync against.", body = FuelSyncOutcomeJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not sync fuel entries, or is an unbound SysAdmin with no tenant named", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
        (status = 503, description = "The fuel-management provider's credential is not configured for this deployment", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn sync(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<FuelSyncOutcomeJson>> {
    let Some(tenant_id) = current_user.tenant_id else {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::FuelEntryForbidden));
    };
    if !can_create_fuel_entry(&current_user, Some(tenant_id)) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::FuelEntryForbidden));
    }

    let provider = configured_provider()
        .map_err(|_| ExceptionResponse::ServiceUnavailable(locale.clone(), ErrorKey::FuelSyncUnavailable))?;
    let db = state.conn.as_ref().clone();
    let use_case = FuelSyncUseCase::new(provider, FuelEntryGateway::new(db.clone()), VehicleGateway::new(db));

    match use_case.sync(Some(tenant_id)).await {
        Ok(outcome) => Ok(Json(FuelSyncOutcomeJson {
            fetched: outcome.fetched,
            imported: outcome.imported,
            updated: outcome.updated,
            unmatched: outcome.unmatched,
        })),
        Err(_) => Err(ExceptionResponse::InternalServerError(
            locale,
            ErrorKey::UnexpectedError,
        )),
    }
}

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct FuelReportQuery {
    /// Inclusive lower bound on the fuelling's timestamp.
    pub from: Option<chrono::NaiveDateTime>,
    /// Inclusive upper bound on the fuelling's timestamp.
    pub to: Option<chrono::NaiveDateTime>,
    pub vehicle_uuid: Option<String>,
    pub station: Option<String>,
}

#[utoipa::path(
    get,
    tag = "FuelEntry",
    path = "/fuel-entry/report",
    params(FuelReportQuery),
    responses(
        (status = 200, description = "The caller's tenant's fuellings inside the filters, oldest first, with litre and value totals (TRM-1553). Average, distance, driver and irregular-only are not included yet (EPIC-FU-05/03). **Roles:** any authenticated role; SysAdmin sees every tenant, everyone else only their own.", body = FuelReportJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 404, description = "The named vehicle was not found, or belongs to another tenant", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn report(
    state: State<AppState>,
    Query(q): Query<FuelReportQuery>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<FuelReportJson>> {
    let vehicle_id = match q.vehicle_uuid {
        Some(uuid) => Some(
            find_visible(&state, &locale, &current_user, uuid)
                .await?
                .id
                .unwrap_or_default(),
        ),
        None => None,
    };
    match use_case(&state).report(q.from, q.to, vehicle_id, q.station).await {
        Ok((entries, total_liters, total_value_cents)) => {
            let mut rows = Vec::with_capacity(entries.len());
            for entry in entries {
                rows.push(json(&state, entry).await);
            }
            Ok(Json(FuelReportJson { entries: rows, total_liters, total_value_cents }))
        }
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}
