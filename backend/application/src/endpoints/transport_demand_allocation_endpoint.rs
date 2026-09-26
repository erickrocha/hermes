use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::json::transport_demand_allocation_json::TransportDemandAllocationJson;
use crate::endpoints::transport_demand_endpoint::{find_visible, resolve_driver_id, resolve_vehicle_id};
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::can_administer_transport_demand;
use business::domain::transport_demand::TransportDemand;
use business::domain::transport_demand_allocation::TransportDemandAllocation;
use business::domain::user::User;
use business::gateway::transport_demand_allocation_gateway::TransportDemandAllocationGateway;
use business::gateway::user_gateway::UserGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::use_cases::transport_demand_allocation_use_case::{
    NOT_A_DRIVER, NOT_A_TENANT_VEHICLE, TransportDemandAllocationUseCase,
};
use business::use_cases::user_use_case::UserUseCase;
use business::use_cases::vehicle_use_case::VehicleUseCase;

fn use_case(state: &AppState) -> TransportDemandAllocationUseCase {
    let db = state.conn.as_ref().clone();
    TransportDemandAllocationUseCase::new(
        TransportDemandAllocationGateway::new(db.clone()),
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

fn require_administrator(
    locale: &Locale,
    user: &User,
    demand: &TransportDemand,
) -> Result<(), ExceptionResponse> {
    if can_administer_transport_demand(user, demand.tenant_id) {
        Ok(())
    } else {
        Err(ExceptionResponse::Forbidden(
            locale.clone(),
            ErrorKey::TransportDemandForbidden,
        ))
    }
}

/// The reverse of `resolve_driver_id`/`resolve_vehicle_id`: an id on the
/// domain object becomes a uuid on the wire. Each lookup is tenant-scoped by
/// the callee's own gateway.
async fn json(
    state: &AppState,
    allocation: TransportDemandAllocation,
) -> TransportDemandAllocationJson {
    let driver_uuid = UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()))
        .find_by_id(allocation.driver_id)
        .await
        .ok()
        .and_then(|u| u.uuid)
        .unwrap_or_default();
    let vehicle_uuid = VehicleUseCase::new(VehicleGateway::new(state.conn.as_ref().clone()))
        .find_by_id(allocation.vehicle_id)
        .await
        .ok()
        .and_then(|v| v.uuid)
        .unwrap_or_default();

    TransportDemandAllocationJson {
        uuid: allocation.uuid,
        driver_uuid,
        vehicle_uuid,
        days_of_week: allocation.days_of_week,
        start_date: allocation.start_date,
        end_date: allocation.end_date,
        active: allocation.active,
    }
}

#[utoipa::path(
    post,
    tag = "Transport Demand",
    path = "/transport-demand/uuid/{uuid}/allocation",
    params(("uuid" = String, Path, description = "Transport demand UUID")),
    request_body = TransportDemandAllocationJson,
    responses(
        (status = 201, description = "The crew is allocated to this demand (HRMS-604). **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only).", body = TransportDemandAllocationJson),
        (status = 400, description = "Bad request, including `NotADriver` or a vehicle outside the demand's own tenant", body = BadRequestErrorJson),
        (status = 404, description = "Demand not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller may read this demand but not allocate its crew", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<TransportDemandAllocationJson>,
) -> HttpResponse<(StatusCode, Json<TransportDemandAllocationJson>)> {
    let demand = find_visible(&state, &locale, &current_user, uuid).await?;
    require_administrator(&locale, &current_user, &demand)?;

    let driver_id = resolve_driver_id(&state, &locale, Some(payload.driver_uuid.clone()))
        .await?
        .ok_or_else(|| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::NotADriver))?;
    let vehicle_id = resolve_vehicle_id(&state, &locale, Some(payload.vehicle_uuid.clone()))
        .await?
        .ok_or_else(|| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue))?;

    let domain = TransportDemandAllocation {
        id: None,
        uuid: None,
        tenant_id: demand.tenant_id,
        demand_id: demand.id.unwrap_or_default(),
        driver_id,
        vehicle_id,
        days_of_week: payload.days_of_week,
        start_date: payload.start_date,
        end_date: payload.end_date,
        active: payload.active,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };

    match use_case(&state).create(domain).await {
        Ok(allocation) => Ok((StatusCode::CREATED, Json(json(&state, allocation).await))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "Transport Demand",
    path = "/transport-demand/uuid/{uuid}/allocations",
    params(("uuid" = String, Path, description = "Transport demand UUID"), PageQuery),
    responses(
        (status = 200, description = "The demand's allocation history, newest first (PD-028). **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = PageJson<TransportDemandAllocationJson>),
        (status = 404, description = "Demand not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
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
) -> HttpResponse<Json<PageJson<TransportDemandAllocationJson>>> {
    let demand = find_visible(&state, &locale, &current_user, uuid).await?;
    let (page, page_size) = (page_query.page(), page_query.page_size());
    match use_case(&state)
        .history(demand.id.unwrap_or_default(), page, page_size)
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

#[utoipa::path(
    put,
    tag = "Transport Demand",
    path = "/transport-demand/uuid/{uuid}/allocation/uuid/{allocation_uuid}",
    params(
        ("uuid" = String, Path, description = "Transport demand UUID"),
        ("allocation_uuid" = String, Path, description = "Allocation UUID"),
    ),
    request_body = TransportDemandAllocationJson,
    responses(
        (status = 200, description = "The allocation is updated -- typically to close its date range or set `active` to false. **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only).", body = TransportDemandAllocationJson),
        (status = 400, description = "Bad request", body = BadRequestErrorJson),
        (status = 404, description = "`TransportDemandNotFound` (also for another tenant's demand, PD-034), or the allocation does not belong to this demand", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller may read this demand but not change its allocation", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn update(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path((uuid, allocation_uuid)): Path<(String, String)>,
    Json(payload): Json<TransportDemandAllocationJson>,
) -> HttpResponse<Json<TransportDemandAllocationJson>> {
    let demand = find_visible(&state, &locale, &current_user, uuid).await?;
    require_administrator(&locale, &current_user, &demand)?;

    let existing = use_case(&state)
        .find_by_uuid(allocation_uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::TransportDemandNotFound))?;
    if existing.demand_id != demand.id.unwrap_or_default() {
        return Err(ExceptionResponse::NotFound(
            locale,
            ErrorKey::TransportDemandNotFound,
        ));
    }

    let driver_id = resolve_driver_id(&state, &locale, Some(payload.driver_uuid.clone()))
        .await?
        .ok_or_else(|| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::NotADriver))?;
    let vehicle_id = resolve_vehicle_id(&state, &locale, Some(payload.vehicle_uuid.clone()))
        .await?
        .ok_or_else(|| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue))?;

    let domain = TransportDemandAllocation {
        id: existing.id,
        uuid: existing.uuid,
        tenant_id: existing.tenant_id,
        demand_id: existing.demand_id,
        driver_id,
        vehicle_id,
        days_of_week: payload.days_of_week,
        start_date: payload.start_date,
        end_date: payload.end_date,
        active: payload.active,
        created_at: existing.created_at,
        created_by: existing.created_by,
        updated_at: None,
        updated_by: None,
    };

    let id = domain.id.unwrap_or_default();
    match use_case(&state).update(id, domain).await {
        Ok(allocation) => Ok(Json(json(&state, allocation).await)),
        Err(e) => Err(error(locale, &e.message)),
    }
}
