use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::json::vehicle_json::VehicleJson;
use crate::infrastructure::mapper::{
    Mapper, VehicleMapper, reject_unknown_garage_tag_origin, reject_unknown_vehicle_status,
};
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::{
    can_administer_vehicle, can_create_vehicle, can_link_tracker_device, can_read_vehicle,
};
use business::domain::user::User;
use business::domain::vehicle::Vehicle;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::domain::vehicle_tracking::VehicleTrackingStatus;
use business::gateway::tracking_provider::configured_provider;
use business::use_cases::vehicle_tracking_use_case::VehicleTrackingUseCase;
use business::use_cases::vehicle_use_case::{
    DUPLICATE_PLATE, DUPLICATE_TRACKER_DEVICE, VehicleUseCase,
};

fn use_case(state: &AppState) -> VehicleUseCase {
    VehicleUseCase::new(VehicleGateway::new(state.conn.as_ref().clone()))
}

/// EPIC-FO-01-S06 (HRMS-925): a plate already registered *in this tenant* is a
/// 409, which is a fact about the caller's own fleet. Every other failure is a
/// 400 -- the use case never reports another tenant's row, because its lookup
/// is tenant-scoped.
fn write_error(locale: Locale, message: &str) -> ExceptionResponse {
    if message == DUPLICATE_PLATE {
        ExceptionResponse::Conflict(locale, ErrorKey::DuplicatePlate)
    } else if message == DUPLICATE_TRACKER_DEVICE {
        ExceptionResponse::Conflict(locale, ErrorKey::DuplicateTrackerDevice)
    } else {
        ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)
    }
}

#[utoipa::path(
    post,
    tag = "Vehicle",
    path = "/vehicle",
    request_body = VehicleJson,
    responses(
        (status = 201, description = "Vehicle registered. **Roles:** SysAdmin (unbound; names the owning tenant); TenantOwner (own tenant only).", body = VehicleJson),
        (status = 400, description = "Bad request, including a status outside the vocabulary (HRMS-922) or a garage tag origin outside Manual/Tracker/Automatic (HRMS-941)", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not register vehicles", body = ForbiddenErrorJson),
        (status = 409, description = "A vehicle with this plate is already registered **in the caller's own tenant** (HRMS-925, D-23(c)). Plates are unique per tenant, never platform-wide: a global rule would disclose another customer's vehicle through this very error.", body = BadRequestErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<VehicleJson>,
) -> HttpResponse<(StatusCode, Json<VehicleJson>)> {
    reject_unknown_vehicle_status(&payload.status, &locale)?;
    reject_unknown_garage_tag_origin(payload.garage_tag_origin.as_deref(), &locale)?;
    let mut domain = VehicleMapper::domain(payload);

    // EPIC-FO-01-S04 (HRMS-923, PD-019): the platform's existing creation
    // hierarchy, not a new one -- a tenant owner registers vehicles in their
    // own tenant, an unbound platform administrator names the tenant.
    if !can_create_vehicle(&current_user, domain.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::VehicleForbidden,
        ));
    }

    // HRMS-926/927: a device id is linked by the platform administrator only.
    if domain.tracker_device_id.is_some() && !can_link_tracker_device(&current_user) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::TrackerLinkForbidden,
        ));
    }

    // HRMS-921: for a tenant-bound caller the owning tenant is the caller's
    // own, never the payload's. `enforce_tenant` (D-06) would overwrite it
    // anyway; setting it here keeps the response honest about what was stored.
    if current_user.tenant_id.is_some() {
        domain.tenant_id = current_user.tenant_id;
    }

    match use_case(&state).create(domain).await {
        Ok(vehicle) => Ok((StatusCode::CREATED, Json(VehicleMapper::json(vehicle)))),
        Err(e) => Err(write_error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "Vehicle",
    path = "/vehicle",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of the caller's own vehicles (PD-028). Another tenant's vehicles are absent rather than forbidden -- the page is filtered by `tenant_select` (HRMS-921, D-09). **Roles:** SysAdmin, TenantOwner, TenantUser, Driver, Mechanic (any authenticated role; SysAdmin sees every tenant, everyone else only their own).", body = PageJson<VehicleJson>),
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
) -> HttpResponse<Json<PageJson<VehicleJson>>> {
    let (page, page_size) = (page_query.page(), page_query.page_size());
    let search = page_query.search();

    // HRMS-921: no second tenant filter here on purpose. Who sees what is the
    // gateway's `tenant_select`, so the list cannot disagree with the single
    // reads about which fleet the caller has -- which is why the authenticated
    // user is not consulted again at this point.

    match use_case(&state)
        .find_page(page, page_size, search.as_deref())
        .await
    {
        Ok((vehicles, total)) => Ok(Json(PageJson::new(
            VehicleMapper::json_vec(vehicles),
            page,
            page_size,
            total,
        ))),
        // DEF-XF-02: an empty page would be indistinguishable from a broken
        // query, so the failure is reported instead of swallowed.
        Err(_) => Err(ExceptionResponse::InternalServerError(
            locale,
            ErrorKey::UnexpectedError,
        )),
    }
}

#[utoipa::path(
    get,
    tag = "Vehicle",
    path = "/vehicle/uuid/{uuid}",
    params(
        ("uuid" = String, Path, description = "Vehicle UUID")
    ),
    responses(
        (status = 200, description = "Vehicle found. **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = VehicleJson),
        (status = 404, description = "Vehicle not found, **or it exists and belongs to another tenant**. PD-034/HRMS-924: a tenant-bound caller is answered 404 rather than 403 on purpose, so the existence of another customer's vehicle is not disclosed. Do not treat this as a defect.", body = NotFoundErrorJson),
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
) -> HttpResponse<Json<VehicleJson>> {
    let vehicle = find_visible(&state, &locale, &current_user, uuid).await?;
    Ok(Json(VehicleMapper::json(vehicle)))
}

#[utoipa::path(
    put,
    tag = "Vehicle",
    path = "/vehicle/uuid/{uuid}",
    params(
        ("uuid" = String, Path, description = "Vehicle UUID")
    ),
    request_body = VehicleJson,
    responses(
        (status = 200, description = "Vehicle updated. **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only).", body = VehicleJson),
        (status = 400, description = "Bad request, including a status outside the vocabulary (HRMS-922) or a garage tag origin outside Manual/Tracker/Automatic (HRMS-941)", body = BadRequestErrorJson),
        (status = 404, description = "Vehicle not found, **or it exists and belongs to another tenant** (PD-034/HRMS-924)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller may read this vehicle but not change it (HRMS-923)", body = ForbiddenErrorJson),
        (status = 409, description = "Another vehicle in the caller's own tenant already carries this plate (HRMS-925)", body = BadRequestErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn update(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<VehicleJson>,
) -> HttpResponse<Json<VehicleJson>> {
    reject_unknown_vehicle_status(&payload.status, &locale)?;
    reject_unknown_garage_tag_origin(payload.garage_tag_origin.as_deref(), &locale)?;
    let existing = find_visible(&state, &locale, &current_user, uuid).await?;

    // HRMS-923: the caller can see this vehicle (it is in their tenant, or
    // they are an unbound administrator) -- 403 here discloses nothing they
    // did not already know, unlike the 404 above.
    if !can_administer_vehicle(&current_user, existing.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::VehicleForbidden,
        ));
    }

    let id = existing.id.unwrap_or_default();
    let mut domain = VehicleMapper::domain(payload);

    // HRMS-926/927: anyone but the platform administrator edits a vehicle
    // with its device link untouched. Omitting the field keeps the link;
    // naming a different device is refused rather than ignored.
    if !can_link_tracker_device(&current_user) {
        if domain.tracker_device_id.is_some()
            && domain.tracker_device_id != existing.tracker_device_id
        {
            return Err(ExceptionResponse::Forbidden(
                locale,
                ErrorKey::TrackerLinkForbidden,
            ));
        }
        domain.tracker_device_id = existing.tracker_device_id;
    }

    match use_case(&state).update(id, domain).await {
        Ok(vehicle) => Ok(Json(VehicleMapper::json(vehicle))),
        Err(e) => Err(write_error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "Vehicle",
    path = "/vehicle/uuid/{uuid}/tracking",
    params(
        ("uuid" = String, Path, description = "Vehicle UUID")
    ),
    responses(
        (status = 200, description = "The vehicle's current tracking status, read live from the provider (HRMS-928). Every answer says when it was reported (`reportedAt`), where it came from (`source`) and whether it may be relied on (`trustworthy`, about ignition). The coordinate carries its own time (`positionReportedAt`) and `positionLive` says whether it shows where the vehicle is now; a stale reading is never passed off as live (HRMS-929). Nothing is stored (HRMS-930). **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = VehicleTrackingStatus),
        (status = 404, description = "`VehicleNotFound`: no such vehicle, **or it belongs to another tenant** (PD-034). `TrackerNotLinked`: the vehicle has no tracking device.", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 503, description = "The tracking provider is not configured or did not answer", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn tracking(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<VehicleTrackingStatus>> {
    // HRMS-927: the position is reached through the caller's own vehicle
    // record, so knowing a device id is not a way into another fleet.
    let vehicle = find_visible(&state, &locale, &current_user, uuid).await?;
    let Some(device_id) = vehicle.tracker_device_id else {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::TrackerNotLinked));
    };
    let unavailable = || ExceptionResponse::ServiceUnavailable(locale.clone(), ErrorKey::TrackingUnavailable);

    // In-request and unpersisted (HRMS-930): the reading is scoped to the
    // vehicle's own tenant (HRMS-903) and handed straight back.
    let provider = configured_provider().map_err(|_| unavailable())?;
    let statuses = VehicleTrackingUseCase::new(provider)
        .ingest_for_tenant(vehicle.tenant_id.unwrap_or_default(), &[device_id])
        .await
        .map_err(|_| unavailable())?;
    statuses.into_iter().next().map(Json).ok_or_else(unavailable)
}

/// PD-034 (HRMS-924, EPIC-FO-01-S05), in one place so no handler can state it
/// differently: a vehicle outside the caller's boundary is **not found**, not
/// forbidden. Two layers agree on that -- the gateway's `tenant_select` makes
/// the row absent for a tenant-bound caller, and `can_read_vehicle` covers an
/// unrestricted read that did return a foreign row.
pub(crate) async fn find_visible(
    state: &AppState,
    locale: &Locale,
    current_user: &User,
    uuid: String,
) -> Result<Vehicle, ExceptionResponse> {
    let vehicle = use_case(state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::VehicleNotFound))?;

    if !can_read_vehicle(current_user, vehicle.tenant_id) {
        return Err(ExceptionResponse::NotFound(
            locale.clone(),
            ErrorKey::VehicleNotFound,
        ));
    }

    Ok(vehicle)
}
