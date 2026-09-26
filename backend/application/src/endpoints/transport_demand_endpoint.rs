use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::json::transport_demand_json::TransportDemandJson;
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::{
    can_administer_transport_demand, can_create_transport_demand, can_read_transport_demand,
};
use business::domain::transport_demand::TransportDemand;
use business::domain::user::User;
use business::gateway::customer_gateway::CustomerGateway;
use business::gateway::transport_demand_gateway::TransportDemandGateway;
use business::gateway::user_gateway::UserGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::use_cases::customer_use_case::CustomerUseCase;
use business::use_cases::transport_demand_use_case::{
    NOT_A_DRIVER, NOT_A_TENANT_VEHICLE, TransportDemandUseCase,
};
use business::use_cases::user_use_case::UserUseCase;
use business::use_cases::vehicle_use_case::VehicleUseCase;

fn use_case(state: &AppState) -> TransportDemandUseCase {
    let db = state.conn.as_ref().clone();
    TransportDemandUseCase::new(
        TransportDemandGateway::new(db.clone()),
        UserGateway::new(db.clone()),
        VehicleGateway::new(db),
    )
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        NOT_A_DRIVER => ExceptionResponse::BadRequest(locale, ErrorKey::NotADriver),
        NOT_A_TENANT_VEHICLE => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

/// `HRMS-204`/`AD-010`: every relation is named by uuid at the boundary and
/// resolved to an internal id here, the one place that happens. `None` is
/// passed through untouched -- these relations are all optional.
pub(crate) async fn resolve_customer_id(
    state: &AppState,
    locale: &Locale,
    tenant_id: Option<i64>,
    uuid: Option<String>,
) -> Result<Option<i64>, ExceptionResponse> {
    let Some(uuid) = uuid else { return Ok(None) };
    let customer = CustomerUseCase::new(CustomerGateway::new(state.conn.as_ref().clone()))
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::CustomerNotFound))?;
    // DEF-FO-01's lesson: an unbound SysAdmin's read is unrestricted, so the
    // tenant match has to be checked here explicitly, not assumed from scope.
    if customer.tenant_id != tenant_id {
        return Err(ExceptionResponse::BadRequest(
            locale.clone(),
            ErrorKey::CustomerNotFound,
        ));
    }
    Ok(customer.id)
}

pub(crate) async fn resolve_driver_id(
    state: &AppState,
    locale: &Locale,
    uuid: Option<String>,
) -> Result<Option<i64>, ExceptionResponse> {
    let Some(uuid) = uuid else { return Ok(None) };
    let user = UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()))
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::NotADriver))?;
    // The role/tenant check itself lives in `TransportDemandUseCase::
    // validated` (`NOT_A_DRIVER`) -- resolving the id is all that happens
    // here, so the rule is stated in exactly one place.
    Ok(user.id)
}

pub(crate) async fn resolve_vehicle_id(
    state: &AppState,
    locale: &Locale,
    uuid: Option<String>,
) -> Result<Option<i64>, ExceptionResponse> {
    let Some(uuid) = uuid else { return Ok(None) };
    let vehicle = VehicleUseCase::new(VehicleGateway::new(state.conn.as_ref().clone()))
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue))?;
    Ok(vehicle.id)
}

async fn resolve_all(
    state: &AppState,
    locale: &Locale,
    tenant_id: Option<i64>,
    payload: &TransportDemandJson,
) -> Result<(Option<i64>, Option<i64>, Option<i64>), ExceptionResponse> {
    let customer_id = resolve_customer_id(state, locale, tenant_id, payload.customer_uuid.clone())
        .await?;
    let driver_id = resolve_driver_id(state, locale, payload.specific_driver_uuid.clone()).await?;
    let vehicle_id = resolve_vehicle_id(state, locale, payload.specific_vehicle_uuid.clone()).await?;
    Ok((customer_id, driver_id, vehicle_id))
}

/// The reverse of `resolve_all`: an id on the domain object becomes a uuid
/// on the wire. Each lookup is tenant-scoped by the callee's own gateway, so
/// this never leaks a foreign row's uuid -- it can only ever resolve ids
/// this very demand already carries.
async fn json(state: &AppState, demand: TransportDemand) -> TransportDemandJson {
    let customer_uuid = match demand.customer_id {
        Some(id) => CustomerUseCase::new(CustomerGateway::new(state.conn.as_ref().clone()))
            .find_by_id(id)
            .await
            .ok()
            .and_then(|c| c.uuid),
        None => None,
    };
    let specific_driver_uuid = match demand.specific_driver_id {
        Some(id) => UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()))
            .find_by_id(id)
            .await
            .ok()
            .and_then(|u| u.uuid),
        None => None,
    };
    let specific_vehicle_uuid = match demand.specific_vehicle_id {
        Some(id) => VehicleUseCase::new(VehicleGateway::new(state.conn.as_ref().clone()))
            .find_by_id(id)
            .await
            .ok()
            .and_then(|v| v.uuid),
        None => None,
    };

    TransportDemandJson {
        id: demand.id,
        uuid: demand.uuid,
        tenant_id: demand.tenant_id,
        demand_type: demand.demand_type,
        customer_uuid,
        line_name: demand.line_name,
        shift_start: demand.shift_start,
        shift_end: demand.shift_end,
        days_of_week: demand.days_of_week,
        specific_date: demand.specific_date,
        priority: demand.priority,
        preferred_vehicle_type: demand.preferred_vehicle_type,
        preferred_vehicle_model: demand.preferred_vehicle_model,
        specific_driver_uuid,
        specific_vehicle_uuid,
        active: demand.active,
    }
}

#[utoipa::path(
    post,
    tag = "Transport Demand",
    path = "/transport-demand",
    request_body = TransportDemandJson,
    responses(
        (status = 201, description = "Demand recorded (HRMS-603). **Roles:** SysAdmin (unbound; names the owning tenant); TenantOwner (own tenant only).", body = TransportDemandJson),
        (status = 400, description = "Bad request, including `NotADriver` (HRMS-603) or a customer/vehicle outside the demand's own tenant", body = crate::endpoints::json::error_response_json::BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = crate::endpoints::json::error_response_json::UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not record demand", body = crate::endpoints::json::error_response_json::ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = crate::endpoints::json::error_response_json::InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<TransportDemandJson>,
) -> HttpResponse<(StatusCode, Json<TransportDemandJson>)> {
    if !can_create_transport_demand(&current_user, payload.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::TransportDemandForbidden,
        ));
    }
    // HRMS-921's rule, applied here: for a tenant-bound caller the owning
    // tenant is the caller's own, never the payload's.
    let tenant_id = current_user.tenant_id.or(payload.tenant_id);

    let (customer_id, specific_driver_id, specific_vehicle_id) =
        resolve_all(&state, &locale, tenant_id, &payload).await?;

    let domain = TransportDemand {
        id: None,
        uuid: None,
        tenant_id,
        demand_type: payload.demand_type,
        customer_id,
        line_name: payload.line_name,
        shift_start: payload.shift_start,
        shift_end: payload.shift_end,
        days_of_week: payload.days_of_week,
        specific_date: payload.specific_date,
        priority: payload.priority,
        preferred_vehicle_type: payload.preferred_vehicle_type,
        preferred_vehicle_model: payload.preferred_vehicle_model,
        specific_driver_id,
        specific_vehicle_id,
        active: payload.active,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };

    match use_case(&state).create(domain).await {
        Ok(demand) => Ok((StatusCode::CREATED, Json(json(&state, demand).await))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "Transport Demand",
    path = "/transport-demand",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of the caller's own tenant's demand (PD-028). **Roles:** SysAdmin, TenantOwner, TenantUser, Driver, Mechanic (any authenticated role; SysAdmin sees every tenant, everyone else only their own).", body = PageJson<TransportDemandJson>),
        (status = 401, description = "Unauthorized", body = crate::endpoints::json::error_response_json::UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = crate::endpoints::json::error_response_json::ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = crate::endpoints::json::error_response_json::InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_all(
    state: State<AppState>,
    Query(page_query): Query<PageQuery>,
    Extension(locale): Extension<Locale>,
    Extension(_current_user): Extension<User>,
) -> HttpResponse<Json<PageJson<TransportDemandJson>>> {
    let (page, page_size) = (page_query.page(), page_query.page_size());
    let search = page_query.search();
    match use_case(&state)
        .find_page(page, page_size, search.as_deref())
        .await
    {
        Ok((demands, total)) => {
            let mut items = Vec::with_capacity(demands.len());
            for demand in demands {
                items.push(json(&state, demand).await);
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
    tag = "Transport Demand",
    path = "/transport-demand/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Transport demand UUID")),
    responses(
        (status = 200, description = "Demand found. **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = TransportDemandJson),
        (status = 404, description = "Demand not found, **or it belongs to another tenant** (PD-034)", body = crate::endpoints::json::error_response_json::NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = crate::endpoints::json::error_response_json::UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = crate::endpoints::json::error_response_json::ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = crate::endpoints::json::error_response_json::InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_by_uuid(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<TransportDemandJson>> {
    let demand = find_visible(&state, &locale, &current_user, uuid).await?;
    Ok(Json(json(&state, demand).await))
}

#[utoipa::path(
    put,
    tag = "Transport Demand",
    path = "/transport-demand/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Transport demand UUID")),
    request_body = TransportDemandJson,
    responses(
        (status = 200, description = "Demand updated. **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only).", body = TransportDemandJson),
        (status = 400, description = "Bad request", body = crate::endpoints::json::error_response_json::BadRequestErrorJson),
        (status = 404, description = "Demand not found, **or it belongs to another tenant** (PD-034)", body = crate::endpoints::json::error_response_json::NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = crate::endpoints::json::error_response_json::UnauthorizedErrorJson),
        (status = 403, description = "The caller may read this demand but not change it", body = crate::endpoints::json::error_response_json::ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = crate::endpoints::json::error_response_json::InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn update(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<TransportDemandJson>,
) -> HttpResponse<Json<TransportDemandJson>> {
    let existing = find_visible(&state, &locale, &current_user, uuid).await?;
    if !can_administer_transport_demand(&current_user, existing.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::TransportDemandForbidden,
        ));
    }

    let (customer_id, specific_driver_id, specific_vehicle_id) =
        resolve_all(&state, &locale, existing.tenant_id, &payload).await?;

    let domain = TransportDemand {
        id: existing.id,
        uuid: existing.uuid,
        tenant_id: existing.tenant_id,
        demand_type: payload.demand_type,
        customer_id,
        line_name: payload.line_name,
        shift_start: payload.shift_start,
        shift_end: payload.shift_end,
        days_of_week: payload.days_of_week,
        specific_date: payload.specific_date,
        priority: payload.priority,
        preferred_vehicle_type: payload.preferred_vehicle_type,
        preferred_vehicle_model: payload.preferred_vehicle_model,
        specific_driver_id,
        specific_vehicle_id,
        active: payload.active,
        created_at: existing.created_at,
        created_by: existing.created_by,
        updated_at: None,
        updated_by: None,
    };

    let id = domain.id.unwrap_or_default();
    match use_case(&state).update(id, domain).await {
        Ok(demand) => Ok(Json(json(&state, demand).await)),
        Err(e) => Err(error(locale, &e.message)),
    }
}

pub(crate) async fn find_visible(
    state: &AppState,
    locale: &Locale,
    current_user: &User,
    uuid: String,
) -> Result<TransportDemand, ExceptionResponse> {
    let demand = use_case(state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::TransportDemandNotFound))?;

    if !can_read_transport_demand(current_user, demand.tenant_id) {
        return Err(ExceptionResponse::NotFound(
            locale.clone(),
            ErrorKey::TransportDemandNotFound,
        ));
    }
    Ok(demand)
}
