use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::json::work_order_item_json::WorkOrderItemJson;
use crate::endpoints::json::work_order_json::WorkOrderJson;
use crate::endpoints::vehicle_endpoint::find_visible;
use crate::infrastructure::mapper::reject_unknown_work_order_item_status;
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::{
    can_administer_work_order, can_create_work_order, can_read_work_order,
};
use business::domain::enums::{WorkOrderItemStatus, WorkOrderOrigin};
use business::domain::user::User;
use business::domain::work_order::WorkOrder;
use business::domain::work_order_item::WorkOrderItem;
use business::gateway::km_evolution_gateway::KmEvolutionGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::gateway::work_order_gateway::WorkOrderGateway;
use business::gateway::work_order_item_gateway::WorkOrderItemGateway;
use business::use_cases::km_evolution_use_case::KmEvolutionUseCase;
use business::use_cases::vehicle_use_case::VehicleUseCase;
use business::use_cases::work_order_use_case::{
    DESCRIPTION_REQUIRED, ITEM_DESCRIPTION_REQUIRED, NEGATIVE_ODOMETER, WORK_ORDER_NOT_FOUND,
    WorkOrderUseCase,
};
use std::str::FromStr;

fn use_case(state: &AppState) -> WorkOrderUseCase {
    let db = state.conn.as_ref().clone();
    WorkOrderUseCase::new(
        WorkOrderGateway::new(db.clone()),
        WorkOrderItemGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        KmEvolutionUseCase::new(KmEvolutionGateway::new(db.clone()), VehicleGateway::new(db)),
    )
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        WORK_ORDER_NOT_FOUND => ExceptionResponse::NotFound(locale, ErrorKey::WorkOrderNotFound),
        DESCRIPTION_REQUIRED | NEGATIVE_ODOMETER | ITEM_DESCRIPTION_REQUIRED => {
            ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)
        }
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

fn item_json(item: WorkOrderItem) -> WorkOrderItemJson {
    WorkOrderItemJson {
        uuid: item.uuid,
        description: item.description,
        item_type: item.item_type,
        status: Some(item.status.to_string()),
        observation: item.observation,
        resolved_by: item.resolved_by,
        resolved_at: item.resolved_at,
        resolution_description: item.resolution_description,
    }
}

async fn json(state: &AppState, work_order: WorkOrder, items: Vec<WorkOrderItem>) -> WorkOrderJson {
    let vehicle_uuid = VehicleUseCase::new(VehicleGateway::new(state.conn.as_ref().clone()))
        .find_by_id(work_order.vehicle_id)
        .await
        .ok()
        .and_then(|v| v.uuid);

    WorkOrderJson {
        uuid: work_order.uuid,
        number: work_order.id.map(|id| format!("OS-{id}")),
        vehicle_uuid: vehicle_uuid.unwrap_or_default(),
        opened_at: work_order.opened_at,
        odometer_km: work_order.odometer_km,
        origin: Some(work_order.origin.to_string()),
        service_type: work_order.service_type,
        description: work_order.description,
        responsible: work_order.responsible,
        status: Some(work_order.status.to_string()),
        observation: work_order.observation,
        external_service: work_order.external_service,
        supplier: work_order.supplier,
        invoice_number: work_order.invoice_number,
        invoice_value_cents: work_order.invoice_value_cents,
        invoice_date: work_order.invoice_date,
        concluded_at: work_order.concluded_at,
        items: items.into_iter().map(item_json).collect(),
    }
}

#[utoipa::path(
    post,
    tag = "WorkOrder",
    path = "/work-order",
    request_body = WorkOrderJson,
    responses(
        (status = 201, description = "The work order is opened, and the vehicle's official odometer record is updated through the one writer EPIC-CK-01-S01 built (HRMS-700, AD-041). `origin` is always `Manual` and `status` is always `Open` regardless of what the caller sends. Tenant is derived from the named vehicle, never sent by the caller. **Roles:** SysAdmin (any tenant); TenantOwner, Mechanic (own tenant only).", body = WorkOrderJson),
        (status = 400, description = "Bad request, including a blank description, a negative odometer reading, or a vehicle outside this tenant", body = BadRequestErrorJson),
        (status = 404, description = "Vehicle not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not open work orders", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<WorkOrderJson>,
) -> HttpResponse<(StatusCode, Json<WorkOrderJson>)> {
    let vehicle = find_visible(&state, &locale, &current_user, payload.vehicle_uuid.clone()).await?;
    if !can_create_work_order(&current_user, vehicle.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::WorkOrderForbidden,
        ));
    }

    let work_order = WorkOrder {
        id: None,
        uuid: None,
        tenant_id: vehicle.tenant_id,
        vehicle_id: vehicle.id.unwrap_or_default(),
        opened_at: payload.opened_at,
        odometer_km: payload.odometer_km,
        origin: WorkOrderOrigin::Manual,
        checklist_run_id: None,
        maintenance_plan_id: None,
        service_type: payload.service_type,
        description: payload.description,
        responsible: payload.responsible,
        status: Default::default(),
        observation: payload.observation,
        external_service: payload.external_service,
        supplier: payload.supplier,
        invoice_number: payload.invoice_number,
        invoice_value_cents: payload.invoice_value_cents,
        invoice_date: payload.invoice_date,
        concluded_at: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };

    match use_case(&state).create(work_order).await {
        Ok(saved) => Ok((StatusCode::CREATED, Json(json(&state, saved, Vec::new()).await))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "WorkOrder",
    path = "/work-order/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Work order UUID")),
    responses(
        (status = 200, description = "The work order. **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = WorkOrderJson),
        (status = 404, description = "Work order not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
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
) -> HttpResponse<Json<WorkOrderJson>> {
    let (work_order, items) = use_case(&state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::WorkOrderNotFound))?;
    if !can_read_work_order(&current_user, work_order.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::WorkOrderNotFound));
    }
    Ok(Json(json(&state, work_order, items).await))
}

#[utoipa::path(
    get,
    tag = "WorkOrder",
    path = "/work-order",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of the caller's own tenant's work orders (PD-028), most recently opened first. **Roles:** SysAdmin, TenantOwner, TenantUser, Driver, Mechanic (any authenticated role; SysAdmin sees every tenant, everyone else only their own).", body = PageJson<WorkOrderJson>),
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
) -> HttpResponse<Json<PageJson<WorkOrderJson>>> {
    let (page, page_size) = (page_query.page(), page_query.page_size());
    match use_case(&state).find_page(page, page_size).await {
        Ok((work_orders, total)) => {
            let mut rows = Vec::with_capacity(work_orders.len());
            for work_order in work_orders {
                rows.push(json(&state, work_order, Vec::new()).await);
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
    tag = "WorkOrder",
    path = "/work-order/uuid/{uuid}/item",
    params(("uuid" = String, Path, description = "Work order UUID")),
    request_body = WorkOrderItemJson,
    responses(
        (status = 201, description = "The item is recorded, and the work order's own status is recomputed from its full item set (HRMS-701, TRM-206/TRM-207) -- unless the order is already Concluded or Cancelled and the new item is not Pending, in which case TRM-215 leaves it untouched. **Roles:** SysAdmin (any tenant); TenantOwner, Mechanic (own tenant only).", body = WorkOrderJson),
        (status = 400, description = "Bad request, including a blank description or an item status outside Pending/Resolved/Cancelled/AwaitingParts", body = BadRequestErrorJson),
        (status = 404, description = "Work order not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not manage this work order", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add_item(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<WorkOrderItemJson>,
) -> HttpResponse<(StatusCode, Json<WorkOrderJson>)> {
    let (work_order, _) = use_case(&state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::WorkOrderNotFound))?;
    if !can_read_work_order(&current_user, work_order.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::WorkOrderNotFound));
    }
    if !can_create_work_order(&current_user, work_order.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::WorkOrderForbidden,
        ));
    }

    let status = payload.status.as_deref().unwrap_or("Pending");
    reject_unknown_work_order_item_status(status, &locale)?;

    let item = WorkOrderItem {
        id: None,
        uuid: None,
        tenant_id: work_order.tenant_id,
        work_order_id: work_order.id.unwrap_or_default(),
        description: payload.description,
        item_type: payload.item_type,
        status: WorkOrderItemStatus::from_str(status).unwrap_or(WorkOrderItemStatus::Pending),
        observation: payload.observation,
        resolved_by: payload.resolved_by,
        resolved_at: payload.resolved_at,
        resolution_description: payload.resolution_description,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };

    match use_case(&state).add_item(work_order.id.unwrap_or_default(), item).await {
        Ok((updated_work_order, saved_item)) => Ok((
            StatusCode::CREATED,
            Json(json(&state, updated_work_order, vec![saved_item]).await),
        )),
        Err(e) => Err(error(locale, &e.message)),
    }
}

/// Both `conclude` and `cancel` share this shape: resolve the uuid, check
/// visibility (404, hiding another tenant's order) then administer rights
/// (403) -- `TRM-208`: conclusion is an administrative act, so this is
/// `can_administer_work_order`, not the wider `can_create_work_order` that
/// lets a `Mechanic` open one or add an item.
async fn administered_work_order(
    state: &AppState,
    locale: &Locale,
    current_user: &User,
    uuid: String,
) -> Result<i64, ExceptionResponse> {
    let (work_order, _) = use_case(state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::WorkOrderNotFound))?;
    if !can_read_work_order(current_user, work_order.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale.clone(), ErrorKey::WorkOrderNotFound));
    }
    if !can_administer_work_order(current_user, work_order.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale.clone(),
            ErrorKey::WorkOrderForbidden,
        ));
    }
    Ok(work_order.id.unwrap_or_default())
}

#[utoipa::path(
    put,
    tag = "WorkOrder",
    path = "/work-order/uuid/{uuid}/conclude",
    params(("uuid" = String, Path, description = "Work order UUID")),
    responses(
        (status = 200, description = "Concluded when every item is Resolved or Cancelled (or there are none); otherwise downgraded to PartiallyResolved with every item left untouched (TRM-204). A work order already Concluded or Cancelled is returned unchanged (TRM-215). **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only) -- conclusion is an administrative act (TRM-208), not open to Mechanic.", body = WorkOrderJson),
        (status = 404, description = "Work order not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not conclude this work order", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn conclude(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<WorkOrderJson>> {
    let work_order_id = administered_work_order(&state, &locale, &current_user, uuid).await?;
    match use_case(&state).conclude(work_order_id).await {
        Ok(work_order) => {
            let items = use_case(&state)
                .find_by_uuid(work_order.uuid.clone().unwrap_or_default())
                .await
                .map(|(_, items)| items)
                .unwrap_or_default();
            Ok(Json(json(&state, work_order, items).await))
        }
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    put,
    tag = "WorkOrder",
    path = "/work-order/uuid/{uuid}/cancel",
    params(("uuid" = String, Path, description = "Work order UUID")),
    responses(
        (status = 200, description = "Cancelled regardless of outstanding items (TRM-205). A work order already Concluded or Cancelled is returned unchanged (TRM-215). **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only) -- the same administrative-act restriction as conclude.", body = WorkOrderJson),
        (status = 404, description = "Work order not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not cancel this work order", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn cancel(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<WorkOrderJson>> {
    let work_order_id = administered_work_order(&state, &locale, &current_user, uuid).await?;
    match use_case(&state).cancel(work_order_id).await {
        Ok(work_order) => {
            let items = use_case(&state)
                .find_by_uuid(work_order.uuid.clone().unwrap_or_default())
                .await
                .map(|(_, items)| items)
                .unwrap_or_default();
            Ok(Json(json(&state, work_order, items).await))
        }
        Err(e) => Err(error(locale, &e.message)),
    }
}
