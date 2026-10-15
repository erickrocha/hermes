use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::maintenance_plan_json::MaintenancePlanJson;
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::vehicle_endpoint::find_visible;
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::commons::gateway::Gateway;
use business::domain::authorization::{can_create_maintenance_plan, can_read_maintenance_plan};
use business::domain::maintenance_plan::MaintenancePlan;
use business::domain::user::User;
use business::domain::work_order::WorkOrder;
use business::gateway::maintenance_plan_gateway::MaintenancePlanGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::gateway::work_order_gateway::WorkOrderGateway;
use business::use_cases::maintenance_plan_use_case::{
    MaintenancePlanUseCase, NOT_A_TENANT_VEHICLE, OVERLAPPING_PLAN_EXISTS, WORK_ORDER_ALREADY_SCHEDULED,
    WORK_ORDER_NOT_FOUND, WORK_ORDER_WRONG_VEHICLE,
};
use business::use_cases::vehicle_use_case::VehicleUseCase;

fn use_case(state: &AppState) -> MaintenancePlanUseCase {
    let db = state.conn.as_ref().clone();
    MaintenancePlanUseCase::new(
        MaintenancePlanGateway::new(db.clone()),
        WorkOrderGateway::new(db.clone()),
        VehicleGateway::new(db),
    )
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        WORK_ORDER_NOT_FOUND => ExceptionResponse::NotFound(locale, ErrorKey::WorkOrderNotFound),
        OVERLAPPING_PLAN_EXISTS | WORK_ORDER_ALREADY_SCHEDULED => {
            ExceptionResponse::Conflict(locale, ErrorKey::InvalidParameterValue)
        }
        NOT_A_TENANT_VEHICLE | WORK_ORDER_WRONG_VEHICLE => {
            ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)
        }
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

async fn resolve_work_order_id(
    state: &AppState,
    locale: &Locale,
    uuid: String,
) -> Result<i64, ExceptionResponse> {
    let work_order = WorkOrderGateway::new(state.conn.as_ref().clone())
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue))?;
    work_order
        .map(|w| w.id)
        .ok_or_else(|| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue))
}

async fn json(state: &AppState, plan: MaintenancePlan, work_orders: Vec<WorkOrder>) -> MaintenancePlanJson {
    let vehicle_uuid = VehicleUseCase::new(VehicleGateway::new(state.conn.as_ref().clone()))
        .find_by_id(plan.vehicle_id)
        .await
        .ok()
        .and_then(|v| v.uuid);

    MaintenancePlanJson {
        uuid: plan.uuid,
        vehicle_uuid: vehicle_uuid.unwrap_or_default(),
        date: plan.date,
        planned_start: plan.planned_start,
        planned_end: plan.planned_end,
        status: Some(plan.status.to_string()),
        affects_schedule: plan.affects_schedule,
        origin: Some(plan.origin),
        work_order_uuids: work_orders.into_iter().filter_map(|w| w.uuid).collect(),
    }
}

#[utoipa::path(
    post,
    tag = "MaintenancePlan",
    path = "/maintenance-plan",
    request_body = MaintenancePlanJson,
    responses(
        (status = 201, description = "The maintenance window is scheduled and every named work order is linked into it (HRMS-703). `status` is always `Scheduled` and `origin` is always `Manual` regardless of what the caller sends. Tenant is derived from the named vehicle, never sent by the caller. **Roles:** SysAdmin (any tenant); TenantOwner, Mechanic (own tenant only).", body = MaintenancePlanJson),
        (status = 400, description = "Bad request, including a vehicle outside this tenant, or a named work order not found or belonging to a different vehicle", body = BadRequestErrorJson),
        (status = 404, description = "Vehicle not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 409, description = "This vehicle already has an active plan for that date (TRM-234), or a named work order is already scheduled into another active plan (TRM-237)", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not schedule maintenance plans", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<MaintenancePlanJson>,
) -> HttpResponse<(StatusCode, Json<MaintenancePlanJson>)> {
    let vehicle = find_visible(&state, &locale, &current_user, payload.vehicle_uuid.clone()).await?;
    if !can_create_maintenance_plan(&current_user, vehicle.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::MaintenancePlanForbidden,
        ));
    }

    let mut work_order_ids = Vec::with_capacity(payload.work_order_uuids.len());
    for uuid in payload.work_order_uuids {
        work_order_ids.push(resolve_work_order_id(&state, &locale, uuid).await?);
    }

    let plan = MaintenancePlan {
        id: None,
        uuid: None,
        tenant_id: vehicle.tenant_id,
        vehicle_id: vehicle.id.unwrap_or_default(),
        date: payload.date,
        planned_start: payload.planned_start,
        planned_end: payload.planned_end,
        status: Default::default(),
        affects_schedule: payload.affects_schedule,
        origin: "Manual".to_string(),
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };

    match use_case(&state).create(plan, work_order_ids).await {
        Ok((saved, work_orders)) => Ok((StatusCode::CREATED, Json(json(&state, saved, work_orders).await))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "MaintenancePlan",
    path = "/maintenance-plan/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Maintenance plan UUID")),
    responses(
        (status = 200, description = "The plan with the work orders it covers. **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = MaintenancePlanJson),
        (status = 404, description = "Plan not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
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
) -> HttpResponse<Json<MaintenancePlanJson>> {
    let (plan, work_orders) = use_case(&state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::MaintenancePlanNotFound))?;
    if !can_read_maintenance_plan(&current_user, plan.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::MaintenancePlanNotFound));
    }
    Ok(Json(json(&state, plan, work_orders).await))
}

#[utoipa::path(
    get,
    tag = "MaintenancePlan",
    path = "/maintenance-plan",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of the caller's own tenant's maintenance plans (PD-028), most recently created first. `workOrderUuids` is always empty here; read a plan by uuid for the work orders it covers. **Roles:** SysAdmin, TenantOwner, TenantUser, Driver, Mechanic (any authenticated role; SysAdmin sees every tenant, everyone else only their own).", body = PageJson<MaintenancePlanJson>),
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
) -> HttpResponse<Json<PageJson<MaintenancePlanJson>>> {
    let (page, page_size) = (page_query.page(), page_query.page_size());
    match use_case(&state).find_page(page, page_size).await {
        Ok((plans, total)) => {
            let mut rows = Vec::with_capacity(plans.len());
            for plan in plans {
                rows.push(json(&state, plan, Vec::new()).await);
            }
            Ok(Json(PageJson::new(rows, page, page_size, total)))
        }
        Err(_) => Err(ExceptionResponse::InternalServerError(
            locale,
            ErrorKey::UnexpectedError,
        )),
    }
}
