use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale, translate_extra_trip_import_error};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::extra_trip_json::ExtraTripJson;
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::json::reference_json::ImportResultJson;
use crate::endpoints::transport_demand_endpoint::{
    resolve_customer_id, resolve_driver_id, resolve_vehicle_id,
};
use crate::infrastructure::mapper::reject_unknown_trip_status;
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::{
    can_administer_extra_trip, can_create_extra_trip, can_read_extra_trip,
};
use business::domain::enums::TripStatus;
use business::domain::extra_trip::ExtraTrip;
use business::domain::user::User;
use business::gateway::customer_gateway::CustomerGateway;
use business::gateway::extra_trip_gateway::ExtraTripGateway;
use business::gateway::user_gateway::UserGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::use_cases::customer_use_case::CustomerUseCase;
use business::use_cases::extra_trip_use_case::{
    DUPLICATE_TRIP, ExtraTripUseCase, NOT_A_DRIVER, NOT_A_TENANT_CUSTOMER, NOT_A_TENANT_VEHICLE,
};
use business::use_cases::user_use_case::UserUseCase;
use business::use_cases::vehicle_use_case::VehicleUseCase;
use std::str::FromStr;

fn use_case(state: &AppState) -> ExtraTripUseCase {
    let db = state.conn.as_ref().clone();
    ExtraTripUseCase::new(
        ExtraTripGateway::new(db.clone()),
        UserGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        CustomerGateway::new(db),
    )
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        DUPLICATE_TRIP => ExceptionResponse::Conflict(locale, ErrorKey::DuplicateTrip),
        NOT_A_DRIVER => ExceptionResponse::BadRequest(locale, ErrorKey::NotADriver),
        NOT_A_TENANT_VEHICLE | NOT_A_TENANT_CUSTOMER => {
            ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)
        }
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

/// Every relation named by uuid, resolved to an internal id here -- the
/// same shape `transport_demand_endpoint::resolve_all` uses, extended with
/// the customer and second-driver seats this entity also carries.
async fn resolve_relations(
    state: &AppState,
    locale: &Locale,
    tenant_id: Option<i64>,
    payload: &ExtraTripJson,
) -> Result<(Option<i64>, Option<i64>, Option<i64>, Option<i64>), ExceptionResponse> {
    let customer_id =
        resolve_customer_id(state, locale, tenant_id, payload.customer_uuid.clone()).await?;
    let driver_id = resolve_driver_id(state, locale, payload.driver_uuid.clone()).await?;
    let second_driver_id =
        resolve_driver_id(state, locale, payload.second_driver_uuid.clone()).await?;
    let vehicle_id = resolve_vehicle_id(state, locale, payload.vehicle_uuid.clone()).await?;
    Ok((customer_id, driver_id, second_driver_id, vehicle_id))
}

/// The reverse: an id on the domain object becomes a uuid on the wire.
/// `replaced_by_id` is resolved through this same use case (self-
/// referential), tenant-scoped by its own gateway like every other lookup.
async fn json(state: &AppState, trip: ExtraTrip) -> ExtraTripJson {
    let customer_uuid = match trip.customer_id {
        Some(id) => CustomerUseCase::new(CustomerGateway::new(state.conn.as_ref().clone()))
            .find_by_id(id)
            .await
            .ok()
            .and_then(|c| c.uuid),
        None => None,
    };
    let driver_uuid = match trip.driver_id {
        Some(id) => UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()))
            .find_by_id(id)
            .await
            .ok()
            .and_then(|u| u.uuid),
        None => None,
    };
    let second_driver_uuid = match trip.second_driver_id {
        Some(id) => UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()))
            .find_by_id(id)
            .await
            .ok()
            .and_then(|u| u.uuid),
        None => None,
    };
    let vehicle_uuid = match trip.vehicle_id {
        Some(id) => VehicleUseCase::new(VehicleGateway::new(state.conn.as_ref().clone()))
            .find_by_id(id)
            .await
            .ok()
            .and_then(|v| v.uuid),
        None => None,
    };
    let replaced_by_uuid = match trip.replaced_by_id {
        Some(id) => use_case(state).find_by_id(id).await.ok().and_then(|t| t.uuid),
        None => None,
    };

    ExtraTripJson {
        id: trip.id,
        uuid: trip.uuid,
        tenant_id: trip.tenant_id,
        order_code: trip.order_code,
        trip_date: trip.trip_date,
        customer_uuid,
        start_time: trip.start_time,
        return_date: trip.return_date,
        return_time: trip.return_time,
        destination: trip.destination,
        origin_city: trip.origin_city,
        stops: trip.stops,
        preferred_vehicle_type: trip.preferred_vehicle_type,
        driver_uuid,
        second_driver_uuid,
        vehicle_uuid,
        freight_value_cents: trip.freight_value_cents,
        payment: trip.payment,
        status: trip.status.to_string(),
        origin: trip.origin,
        import_batch_id: trip.import_batch_id,
        imported_at: trip.imported_at,
        replaced_by_uuid,
    }
}

#[utoipa::path(
    post,
    tag = "Extra Trip",
    path = "/extra-trip",
    request_body = ExtraTripJson,
    responses(
        (status = 201, description = "Trip recorded (HRMS-607, D-24(f)). **Roles:** SysAdmin (unbound; names the owning tenant); TenantOwner (own tenant only).", body = ExtraTripJson),
        (status = 400, description = "Bad request, including a status outside Scheduled/Conflict/PendingSchedule/Cancelled (HRMS-607), `NotADriver`, or a customer/vehicle outside the trip's own tenant", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not record trips", body = ForbiddenErrorJson),
        (status = 409, description = "`DuplicateTrip`: this tenant already has a trip with this order code on this date (D-24(d))", body = BadRequestErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<ExtraTripJson>,
) -> HttpResponse<(StatusCode, Json<ExtraTripJson>)> {
    if !can_create_extra_trip(&current_user, payload.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::ExtraTripForbidden,
        ));
    }
    reject_unknown_trip_status(&payload.status, &locale)?;
    // HRMS-921's rule, applied here: for a tenant-bound caller the owning
    // tenant is the caller's own, never the payload's.
    let tenant_id = current_user.tenant_id.or(payload.tenant_id);

    let (customer_id, driver_id, second_driver_id, vehicle_id) =
        resolve_relations(&state, &locale, tenant_id, &payload).await?;

    let domain = ExtraTrip {
        id: None,
        uuid: None,
        tenant_id,
        order_code: payload.order_code,
        trip_date: payload.trip_date,
        customer_id,
        start_time: payload.start_time,
        return_date: payload.return_date,
        return_time: payload.return_time,
        destination: payload.destination,
        origin_city: payload.origin_city,
        stops: payload.stops,
        preferred_vehicle_type: payload.preferred_vehicle_type,
        driver_id,
        second_driver_id,
        vehicle_id,
        freight_value_cents: payload.freight_value_cents,
        payment: payload.payment,
        status: TripStatus::from_str(&payload.status).unwrap_or_default(),
        origin: payload.origin,
        import_batch_id: payload.import_batch_id,
        imported_at: payload.imported_at,
        replaced_by_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };

    match use_case(&state).create(domain).await {
        Ok(trip) => Ok((StatusCode::CREATED, Json(json(&state, trip).await))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "Extra Trip",
    path = "/extra-trip",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of the caller's own tenant's trips (PD-028), latest date first. **Roles:** SysAdmin, TenantOwner, TenantUser, Driver, Mechanic (any authenticated role; SysAdmin sees every tenant, everyone else only their own).", body = PageJson<ExtraTripJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_all(
    state: State<AppState>,
    Query(page_query): Query<PageQuery>,
    Extension(locale): Extension<Locale>,
    Extension(_current_user): Extension<User>,
) -> HttpResponse<Json<PageJson<ExtraTripJson>>> {
    let (page, page_size) = (page_query.page(), page_query.page_size());
    let search = page_query.search();
    match use_case(&state)
        .find_page(page, page_size, search.as_deref())
        .await
    {
        Ok((trips, total)) => {
            let mut items = Vec::with_capacity(trips.len());
            for trip in trips {
                items.push(json(&state, trip).await);
            }
            Ok(Json(PageJson::new(items, page, page_size, total)))
        }
        Err(_) => Err(ExceptionResponse::InternalServerError(
            locale,
            ErrorKey::UnexpectedError,
        )),
    }
}

#[utoipa::path(
    get,
    tag = "Extra Trip",
    path = "/extra-trip/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Trip UUID")),
    responses(
        (status = 200, description = "Trip found. **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = ExtraTripJson),
        (status = 404, description = "Trip not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_by_uuid(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<ExtraTripJson>> {
    let trip = find_visible(&state, &locale, &current_user, uuid).await?;
    Ok(Json(json(&state, trip).await))
}

#[utoipa::path(
    put,
    tag = "Extra Trip",
    path = "/extra-trip/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Trip UUID")),
    request_body = ExtraTripJson,
    responses(
        (status = 200, description = "Trip updated. **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only).", body = ExtraTripJson),
        (status = 400, description = "Bad request", body = BadRequestErrorJson),
        (status = 404, description = "Trip not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller may read this trip but not change it", body = ForbiddenErrorJson),
        (status = 409, description = "`DuplicateTrip`: another trip in this tenant already carries this order code on this date", body = BadRequestErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn update(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<ExtraTripJson>,
) -> HttpResponse<Json<ExtraTripJson>> {
    let existing = find_visible(&state, &locale, &current_user, uuid).await?;
    if !can_administer_extra_trip(&current_user, existing.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::ExtraTripForbidden,
        ));
    }
    reject_unknown_trip_status(&payload.status, &locale)?;

    let (customer_id, driver_id, second_driver_id, vehicle_id) =
        resolve_relations(&state, &locale, existing.tenant_id, &payload).await?;

    let domain = ExtraTrip {
        id: existing.id,
        uuid: existing.uuid,
        tenant_id: existing.tenant_id,
        order_code: payload.order_code,
        trip_date: payload.trip_date,
        customer_id,
        start_time: payload.start_time,
        return_date: payload.return_date,
        return_time: payload.return_time,
        destination: payload.destination,
        origin_city: payload.origin_city,
        stops: payload.stops,
        preferred_vehicle_type: payload.preferred_vehicle_type,
        driver_id,
        second_driver_id,
        vehicle_id,
        freight_value_cents: payload.freight_value_cents,
        payment: payload.payment,
        status: TripStatus::from_str(&payload.status).unwrap_or_default(),
        origin: payload.origin,
        import_batch_id: payload.import_batch_id,
        imported_at: payload.imported_at,
        replaced_by_id: existing.replaced_by_id,
        created_at: existing.created_at,
        created_by: existing.created_by,
        updated_at: None,
        updated_by: None,
    };

    let id = domain.id.unwrap_or_default();
    match use_case(&state).update(id, domain).await {
        Ok(trip) => Ok(Json(json(&state, trip).await)),
        Err(e) => Err(error(locale, &e.message)),
    }
}

async fn find_visible(
    state: &AppState,
    locale: &Locale,
    current_user: &User,
    uuid: String,
) -> Result<ExtraTrip, ExceptionResponse> {
    let trip = use_case(state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::ExtraTripNotFound))?;

    if !can_read_extra_trip(current_user, trip.tenant_id) {
        return Err(ExceptionResponse::NotFound(
            locale.clone(),
            ErrorKey::ExtraTripNotFound,
        ));
    }
    Ok(trip)
}

/// `EPIC-SC-03-S02` (`HRMS-608`, `PD-027`): the console parses and lets the
/// operator edit the CSV before this is called, the same shape `/city/
/// import` and `/province/import` already use. Each row's relations are
/// resolved and its tenant checked *before* the use case ever runs, so a bad
/// uuid or a role the caller may not act as aborts the whole batch with a
/// plain 400/403 -- itemised, per-row rejection (`ImportResultJson`'s
/// counterpart) is reserved for the *business* rules `ExtraTripUseCase::
/// import` itself checks (driver/vehicle/customer tenant, D-24(d) identity).
#[utoipa::path(
    post,
    tag = "Extra Trip",
    path = "/extra-trip/import",
    request_body = Vec<ExtraTripJson>,
    responses(
        (status = 200, description = "Import applied (HRMS-608). **Roles:** SysAdmin (unbound; every row must name its tenant); TenantOwner (own tenant only).", body = ImportResultJson),
        (status = 400, description = "Nothing was written; the message names each rejected row, or a row's uuid/status could not be resolved", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not record trips for a row's tenant", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn import(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<Vec<ExtraTripJson>>,
) -> HttpResponse<Json<ImportResultJson>> {
    let mut rows = Vec::with_capacity(payload.len());
    for item in payload {
        let tenant_id = current_user.tenant_id.or(item.tenant_id);
        if !can_create_extra_trip(&current_user, tenant_id) {
            return Err(ExceptionResponse::Forbidden(
                locale,
                ErrorKey::ExtraTripForbidden,
            ));
        }
        reject_unknown_trip_status(&item.status, &locale)?;

        let (customer_id, driver_id, second_driver_id, vehicle_id) =
            resolve_relations(&state, &locale, tenant_id, &item).await?;

        rows.push(ExtraTrip {
            id: None,
            uuid: None,
            tenant_id,
            order_code: item.order_code,
            trip_date: item.trip_date,
            customer_id,
            start_time: item.start_time,
            return_date: item.return_date,
            return_time: item.return_time,
            destination: item.destination,
            origin_city: item.origin_city,
            stops: item.stops,
            preferred_vehicle_type: item.preferred_vehicle_type,
            driver_id,
            second_driver_id,
            vehicle_id,
            freight_value_cents: item.freight_value_cents,
            payment: item.payment,
            status: TripStatus::from_str(&item.status).unwrap_or_default(),
            origin: item.origin,
            import_batch_id: item.import_batch_id,
            imported_at: item.imported_at,
            replaced_by_id: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        });
    }

    use_case(&state)
        .import(rows)
        .await
        .map(|outcome| {
            Json(ImportResultJson {
                created: outcome.created,
                updated: outcome.updated,
            })
        })
        .map_err(|error| ExceptionResponse::BadRequestMessage(translate_extra_trip_import_error(&locale, &error)))
}
