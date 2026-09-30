use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::fuel_entry_json::{
    ConsumptionAverageJson, FuelEntryJson, FuelReceiptJson, FuelReportJson, FuelSyncOutcomeJson, FuelUnifyJson, TankGaugeJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::vehicle_endpoint::find_visible;
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::commons::functions::bytes_para_string;
use business::commons::gateway::Gateway;
use business::domain::authorization::{can_create_fuel_entry, can_read_fuel_entry, can_report_fuel_receipt};
use business::domain::enums::Role;
use business::use_cases::fuel_entry_use_case::{
    NOTHING_TO_RESTORE, UNIFY_DIFFERENT_DAYS, UNIFY_DIFFERENT_VEHICLES, UNIFY_NEEDS_TWO_ENTRIES,
};
use business::use_cases::fuel_receipt_use_case::{
    FuelReceipt, FuelReceiptUseCase, NOT_A_DRIVER, ODOMETER_OUTSIDE_WINDOW, PREFIX_MISMATCH,
};
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
    FuelEntryUseCase::new(
        FuelEntryGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        business::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway::new(db),
    )
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
        reported_by_user_id: None,
        odometer_override_note: None,
        provider_confirmed_at: None,
        deleted_at: None,
        unified_into_id: None,
        unification_note: None,
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
    let use_case = FuelSyncUseCase::new(
        provider,
        FuelEntryGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        business::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway::new(db),
    );

    match use_case.sync(Some(tenant_id)).await {
        Ok(outcome) => Ok(Json(FuelSyncOutcomeJson {
            fetched: outcome.fetched,
            imported: outcome.imported,
            updated: outcome.updated,
            reconciled: outcome.reconciled,
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

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct FuelAverageQuery {
    pub vehicle_uuid: String,
}

#[utoipa::path(
    get,
    tag = "FuelEntry",
    path = "/fuel-entry/average",
    params(FuelAverageQuery),
    responses(
        (status = 200, description = "The vehicle's own consumption average from full-tank-to-full-tank segments (TRM-563…568): partial fuellings fold into the open segment, implausible segments (over 4,000 km, or outside 0.8–15 km/l) are dropped, the last 30 days' segments (else the last 8) are kept, those over 40 % from the median are rejected while at least 2 survive, and the figure is total distance over total litres. `kmPerLiter` is absent until 3 segments back it. **Roles:** any authenticated role; SysAdmin sees every tenant, everyone else only their own.", body = ConsumptionAverageJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 404, description = "The vehicle was not found, or belongs to another tenant", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn average(
    state: State<AppState>,
    Query(q): Query<FuelAverageQuery>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<ConsumptionAverageJson>> {
    let vehicle = find_visible(&state, &locale, &current_user, q.vehicle_uuid).await?;
    use business::use_cases::fuel_gauge_use_case::{ConsumptionSource, FuelGaugeUseCase};
    let vehicle_id = vehicle.id.unwrap_or_default();
    let db = state.conn.as_ref().clone();
    let effective = FuelGaugeUseCase::new(
        FuelEntryGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        business::gateway::km_evolution_gateway::KmEvolutionGateway::new(db.clone()),
        business::gateway::fuel_gauge_setting_gateway::FuelGaugeSettingGateway::new(db.clone()),
        business::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway::new(db),
    )
    .effective_consumption(vehicle_id)
    .await;
    match (use_case(&state).average(vehicle_id).await, effective) {
        (Ok(a), Ok(e)) => Ok(Json(ConsumptionAverageJson {
            km_per_liter: a.km_per_liter,
            segments: a.segments,
            mature: a.mature,
            effective_km_per_liter: e.km_per_liter,
            effective_source: e.source.map(|s| {
                match s {
                    ConsumptionSource::Own => "Own",
                    ConsumptionSource::Reference => "Reference",
                    ConsumptionSource::PeerMedian => "PeerMedian",
                }
                .to_string()
            }),
        })),
        _ => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}

#[utoipa::path(
    post,
    tag = "FuelEntry",
    path = "/fuel-entry/receipt",
    request_body = FuelReceiptJson,
    responses(
        (status = 201, description = "A fuelling the driver confirmed from a receipt is recorded at once, without waiting for the provider (TRM-520/542): origin `DriverPhoto`, no provider transaction, the reporting driver kept. **Roles:** Driver (as themselves); TenantOwner, Mechanic (on a named driver's behalf); SysAdmin (unbound).", body = FuelEntryJson),
        (status = 200, description = "The same vehicle already has a fuelling within 0.5 l inside 10 minutes of this one; that record is returned and nothing is created (TRM-541)", body = FuelEntryJson),
        (status = 400, description = "Non-positive volume, negative value, a prefix that does not match the vehicle (TRM-536), or the reporter is not an active driver of this tenant (TRM-535)", body = BadRequestErrorJson),
        (status = 409, description = "`OdometerOutsideWindow`: the odometer lies outside the readings immediately before and after the receipt's own date (TRM-538/539); the message carries the reference and both bounds. Resubmit with `overrideOdometer: true` to record it with the override noted (TRM-540)", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not report a fuelling", body = ForbiddenErrorJson),
        (status = 404, description = "Vehicle not found, or in another tenant", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn receipt(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<FuelReceiptJson>,
) -> HttpResponse<(StatusCode, Json<FuelEntryJson>)> {
    let vehicle = find_visible(&state, &locale, &current_user, payload.vehicle_uuid).await?;
    if !can_report_fuel_receipt(&current_user, vehicle.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::FuelEntryForbidden));
    }
    let driver_id = if current_user.role == Role::Driver {
        current_user.id
    } else {
        crate::endpoints::transport_demand_endpoint::resolve_driver_id(&state, &locale, payload.driver_uuid).await?
    };
    let Some(driver_id) = driver_id else {
        return Err(ExceptionResponse::BadRequest(locale, ErrorKey::NotADriver));
    };

    let db = state.conn.as_ref().clone();
    let receipt_use_case = FuelReceiptUseCase::new(
        FuelEntryGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        business::gateway::user_gateway::UserGateway::new(db.clone()),
        business::gateway::km_evolution_gateway::KmEvolutionGateway::new(db.clone()),
        business::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway::new(db),
    );
    let outcome = receipt_use_case
        .record(FuelReceipt {
            tenant_id: vehicle.tenant_id,
            vehicle_id: vehicle.id.unwrap_or_default(),
            driver_id,
            confirmed_prefix: payload.vehicle_prefix,
            recorded_at: payload.recorded_at,
            volume_liters: payload.volume_liters,
            value_cents: payload.value_cents,
            odometer_km: payload.odometer_km,
            station: payload.station,
            full_tank: payload.full_tank,
            override_odometer: payload.override_odometer,
        })
        .await;
    match outcome {
        Ok((entry, created)) => {
            let status = if created { StatusCode::CREATED } else { StatusCode::OK };
            Ok((status, Json(json(&state, entry).await)))
        }
        Err(e) if e.message.starts_with(ODOMETER_OUTSIDE_WINDOW) => {
            Err(ExceptionResponse::Conflict(locale, ErrorKey::OdometerOutsideWindow))
        }
        Err(e) if e.message == NOT_A_DRIVER => Err(ExceptionResponse::BadRequest(locale, ErrorKey::NotADriver)),
        Err(e) if e.message == PREFIX_MISMATCH => {
            Err(ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue))
        }
        Err(e) => Err(error(locale, &e.message)),
    }
}

async fn administered(
    state: &AppState,
    locale: &Locale,
    current_user: &User,
    uuid: &str,
) -> Result<FuelEntry, ExceptionResponse> {
    let entry = use_case(state)
        .find_by_uuid(uuid.to_string())
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::FuelEntryNotFound))?;
    if !can_read_fuel_entry(current_user, entry.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale.clone(), ErrorKey::FuelEntryNotFound));
    }
    if !can_create_fuel_entry(current_user, entry.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale.clone(), ErrorKey::FuelEntryForbidden));
    }
    Ok(entry)
}

#[utoipa::path(
    delete,
    tag = "FuelEntry",
    path = "/fuel-entry/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Fuel entry UUID")),
    responses(
        (status = 204, description = "The fuelling is soft-deleted (TRM-552): every list, report, average and tank figure stops counting it; it can be brought back with `POST /fuel-entry/restore`. **Roles:** SysAdmin, TenantOwner, Mechanic (own tenant only)."),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not change the fuel ledger", body = ForbiddenErrorJson),
        (status = 404, description = "Fuel entry not found, **or in another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<StatusCode> {
    administered(&state, &locale, &current_user, &uuid).await?;
    match use_case(&state).soft_delete(uuid).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct FuelRestoreQuery {
    pub vehicle_uuid: String,
}

#[utoipa::path(
    post,
    tag = "FuelEntry",
    path = "/fuel-entry/restore",
    params(FuelRestoreQuery),
    responses(
        (status = 200, description = "The vehicle's most recently deleted fuelling is restored with all its data intact (TRM-553). One absorbed by a unification is never the one restored. **Roles:** SysAdmin, TenantOwner, Mechanic (own tenant only).", body = FuelEntryJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not change the fuel ledger", body = ForbiddenErrorJson),
        (status = 404, description = "Vehicle not found, or it has no deleted fuelling", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn restore(
    state: State<AppState>,
    Query(q): Query<FuelRestoreQuery>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<FuelEntryJson>> {
    let vehicle = find_visible(&state, &locale, &current_user, q.vehicle_uuid).await?;
    if !can_create_fuel_entry(&current_user, vehicle.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::FuelEntryForbidden));
    }
    match use_case(&state).restore_last(vehicle.id.unwrap_or_default()).await {
        Ok(entry) => Ok(Json(json(&state, entry).await)),
        Err(e) if e.message == NOTHING_TO_RESTORE => {
            Err(ExceptionResponse::NotFound(locale, ErrorKey::FuelEntryNotFound))
        }
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}

#[utoipa::path(
    post,
    tag = "FuelEntry",
    path = "/fuel-entry/unify",
    request_body = FuelUnifyJson,
    responses(
        (status = 200, description = "Two fuellings of the same vehicle and day become one (TRM-554): the target keeps the summed volume and value, takes the source's odometer if it has none, and is a full tank if either was; the source is soft-deleted and the target annotated. **Roles:** SysAdmin, TenantOwner, Mechanic (own tenant only).", body = FuelEntryJson),
        (status = 400, description = "The same entry twice, different vehicles, or different days", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not change the fuel ledger", body = ForbiddenErrorJson),
        (status = 404, description = "An entry was not found, **or is in another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn unify(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<FuelUnifyJson>,
) -> HttpResponse<Json<FuelEntryJson>> {
    administered(&state, &locale, &current_user, &payload.target_uuid).await?;
    administered(&state, &locale, &current_user, &payload.source_uuid).await?;
    match use_case(&state).unify(payload.target_uuid, payload.source_uuid).await {
        Ok(entry) => Ok(Json(json(&state, entry).await)),
        Err(e)
            if matches!(
                e.message.as_str(),
                UNIFY_NEEDS_TWO_ENTRIES | UNIFY_DIFFERENT_VEHICLES | UNIFY_DIFFERENT_DAYS
            ) =>
        {
            Err(ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue))
        }
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct FuelGaugeQuery {
    pub vehicle_uuid: String,
}

#[utoipa::path(
    get,
    tag = "FuelEntry",
    path = "/fuel-entry/gauge",
    params(FuelGaugeQuery),
    responses(
        (status = 200, description = "The vehicle's estimated tank level (TRM-580): registered capacity less the distance run since its latest full-tank fuelling (by its own datetime) over its own consumption average, with partial fuellings since credited back. Distance is the official odometer record's, never a sum of trips (TRM-583). With no usable reading `available` is false and `reason` says which of four applies (TRM-597); a reading implying more than 105 % of the tank is `Suspect`, never a confident zero (TRM-596). **Roles:** any authenticated role; SysAdmin sees every tenant, everyone else only their own.", body = TankGaugeJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 404, description = "The vehicle was not found, or belongs to another tenant", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn gauge(
    state: State<AppState>,
    Query(q): Query<FuelGaugeQuery>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<TankGaugeJson>> {
    use business::use_cases::fuel_gauge_use_case::{FuelGaugeUseCase, GaugeUnavailable, TankGauge};
    let vehicle = find_visible(&state, &locale, &current_user, q.vehicle_uuid).await?;
    let db = state.conn.as_ref().clone();
    let gauge = FuelGaugeUseCase::new(
        FuelEntryGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        business::gateway::km_evolution_gateway::KmEvolutionGateway::new(db.clone()),
        business::gateway::fuel_gauge_setting_gateway::FuelGaugeSettingGateway::new(db.clone()),
        business::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway::new(db),
    )
    .gauge(vehicle.id.unwrap_or_default())
    .await
    .map_err(|_| ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError))?;
    let empty = TankGaugeJson {
        available: false,
        percent: None,
        remaining_liters: None,
        range_km: None,
        consumption_km_per_liter: None,
        anchored_at: None,
        estimated: false,
        set_aside_at: Vec::new(),
        reason: None,
    };
    Ok(Json(match gauge {
        TankGauge::Reading { percent, remaining_liters, range_km, consumption_km_per_liter, anchored_at, estimated, set_aside_at } => TankGaugeJson {
            available: true,
            percent: Some(percent),
            remaining_liters: Some(remaining_liters),
            range_km: Some(range_km),
            consumption_km_per_liter: Some(consumption_km_per_liter),
            anchored_at: Some(anchored_at),
            estimated,
            set_aside_at,
            ..empty
        },
        TankGauge::Unavailable(why) => TankGaugeJson {
            reason: Some(
                match why {
                    GaugeUnavailable::NoTankRegistered => "NoTankRegistered",
                    GaugeUnavailable::NoReferenceFuelling => "NoReferenceFuelling",
                    GaugeUnavailable::AwaitingCalibration => "AwaitingCalibration",
                    GaugeUnavailable::Suspect => "Suspect",
                }
                .to_string(),
            ),
            ..empty
        },
    }))
}
