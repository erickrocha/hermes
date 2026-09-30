use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::json::purchase_order_json::{MarkOrderedJson, PurchaseOrderJson};
use crate::endpoints::vehicle_endpoint::find_visible as find_visible_vehicle;
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::{
    can_administer_purchase_order, can_create_purchase_order, can_read_part, can_read_purchase_order,
    can_read_work_order,
};
use business::domain::part::Part;
use business::domain::purchase_order::PurchaseOrder;
use business::domain::user::User;
use business::commons::functions::bytes_para_string;
use business::commons::gateway::Gateway;
use business::gateway::km_evolution_gateway::KmEvolutionGateway;
use business::gateway::part_gateway::PartGateway;
use business::gateway::purchase_order_gateway::PurchaseOrderGateway;
use business::gateway::preventive_plan_gateway::PreventivePlanGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::gateway::work_order_gateway::WorkOrderGateway;
use business::gateway::work_order_item_gateway::WorkOrderItemGateway;
use business::use_cases::km_evolution_use_case::KmEvolutionUseCase;
use business::use_cases::part_use_case::PartUseCase;
use business::use_cases::purchase_order_use_case::{
    DUPLICATE_ACTIVE_PURCHASE_ORDER, PART_NOT_FOUND, PurchaseOrderUseCase, QUANTITY_MUST_BE_POSITIVE,
    WRONG_STATUS_FOR_TRANSITION,
};
use business::use_cases::work_order_use_case::WorkOrderUseCase;

fn use_case(state: &AppState) -> PurchaseOrderUseCase {
    let db = state.conn.as_ref().clone();
    PurchaseOrderUseCase::new(
        PurchaseOrderGateway::new(db.clone()),
        PartGateway::new(db.clone()),
        work_order_use_case(state),
    )
}

fn part_use_case(state: &AppState) -> PartUseCase {
    PartUseCase::new(PartGateway::new(state.conn.as_ref().clone()))
}

fn work_order_use_case(state: &AppState) -> WorkOrderUseCase {
    let db = state.conn.as_ref().clone();
    WorkOrderUseCase::new(
        WorkOrderGateway::new(db.clone()),
        WorkOrderItemGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        KmEvolutionUseCase::new(KmEvolutionGateway::new(db.clone()), VehicleGateway::new(db.clone())),
        PreventivePlanGateway::new(db.clone()),
        business::gateway::preventive_plan_extension_gateway::PreventivePlanExtensionGateway::new(db),
    )
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        PART_NOT_FOUND => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
        QUANTITY_MUST_BE_POSITIVE | DUPLICATE_ACTIVE_PURCHASE_ORDER | WRONG_STATUS_FOR_TRANSITION => {
            ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)
        }
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

async fn resolve_part(
    state: &AppState,
    locale: &Locale,
    current_user: &User,
    uuid: String,
) -> Result<Part, ExceptionResponse> {
    let part = part_use_case(state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::PartNotFound))?;
    if !can_read_part(current_user, part.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale.clone(), ErrorKey::PartNotFound));
    }
    Ok(part)
}

/// Resolves an optional work-order uuid to its id, hiding another tenant's
/// order the same way `resolve_part`/`find_visible_vehicle` hide theirs.
async fn resolve_work_order_id(
    state: &AppState,
    locale: &Locale,
    current_user: &User,
    uuid: Option<String>,
) -> Result<Option<i64>, ExceptionResponse> {
    let Some(uuid) = uuid else { return Ok(None) };
    let (work_order, _) = work_order_use_case(state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::WorkOrderNotFound))?;
    if !can_read_work_order(current_user, work_order.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale.clone(), ErrorKey::WorkOrderNotFound));
    }
    Ok(work_order.id)
}

/// `TRM-644`: resolves the named pendency's uuid to its id, scoped to this
/// work order the same way `mark_item_awaiting_parts` itself checks.
/// `find_by_id` (via `Gateway`) already applies tenant scoping, so a found
/// item is by construction visible to the caller.
async fn resolve_work_order_item_id(
    state: &AppState,
    locale: &Locale,
    uuid: Option<String>,
) -> Result<Option<i64>, ExceptionResponse> {
    let Some(uuid) = uuid else { return Ok(None) };
    let item = WorkOrderItemGateway::new(state.conn.as_ref().clone())
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::WorkOrderNotFound))?
        .ok_or_else(|| ExceptionResponse::NotFound(locale.clone(), ErrorKey::WorkOrderNotFound))?;
    Ok(Some(item.id))
}

async fn resolve_vehicle_id(
    state: &AppState,
    locale: &Locale,
    current_user: &User,
    uuid: Option<String>,
) -> Result<Option<i64>, ExceptionResponse> {
    let Some(uuid) = uuid else { return Ok(None) };
    let vehicle = find_visible_vehicle(state, locale, current_user, uuid).await?;
    Ok(vehicle.id)
}

async fn json(state: &AppState, purchase_order: PurchaseOrder) -> PurchaseOrderJson {
    PurchaseOrderJson {
        uuid: purchase_order.uuid,
        number: purchase_order.id.map(|id| format!("PO-{id}")),
        tenant_id: purchase_order.tenant_id,
        part_uuid: part_uuid_of(state, purchase_order.part_id).await,
        quantity: purchase_order.quantity,
        suggested_supplier: purchase_order.suggested_supplier,
        observation: purchase_order.observation,
        status: Some(purchase_order.status.to_string()),
        work_order_uuid: match purchase_order.work_order_id {
            Some(id) => work_order_uuid_of(state, id).await,
            None => None,
        },
        work_order_item_uuid: match purchase_order.work_order_item_id {
            Some(id) => work_order_item_uuid_of(state, id).await,
            None => None,
        },
        vehicle_uuid: match purchase_order.vehicle_id {
            Some(id) => vehicle_uuid_of(state, id).await,
            None => None,
        },
        ordered_at: purchase_order.ordered_at,
        expected_delivery_date: purchase_order.expected_delivery_date,
    }
}

async fn part_uuid_of(state: &AppState, part_id: i64) -> String {
    PartGateway::new(state.conn.as_ref().clone())
        .find_by_id(part_id)
        .await
        .ok()
        .flatten()
        .map(|m| bytes_para_string(m.uuid))
        .unwrap_or_default()
}

async fn work_order_uuid_of(state: &AppState, work_order_id: i64) -> Option<String> {
    WorkOrderGateway::new(state.conn.as_ref().clone())
        .find_by_id(work_order_id)
        .await
        .ok()
        .flatten()
        .map(|m| bytes_para_string(m.uuid))
}

async fn work_order_item_uuid_of(state: &AppState, item_id: i64) -> Option<String> {
    WorkOrderItemGateway::new(state.conn.as_ref().clone())
        .find_by_id(item_id)
        .await
        .ok()
        .flatten()
        .map(|m| bytes_para_string(m.uuid))
}

async fn vehicle_uuid_of(state: &AppState, vehicle_id: i64) -> Option<String> {
    VehicleGateway::new(state.conn.as_ref().clone())
        .find_by_id(vehicle_id)
        .await
        .ok()
        .flatten()
        .map(|m| bytes_para_string(m.uuid))
}

#[utoipa::path(
    post,
    tag = "PurchaseOrder",
    path = "/purchase-order",
    request_body = PurchaseOrderJson,
    responses(
        (status = 201, description = "The purchase order is raised (TRM-640), always `Requested` (TRM-641) regardless of what the caller sends. `number` (`PO-{id}`) is server-derived, a distinct prefix from `Part`'s own `PC-{id}` (closing U-170 by construction). Refused when this work order already has an active order for this part (TRM-642). When `workOrderUuid` is given, naming `workOrderItemUuid` links that existing pendency (TRM-644); naming none instead opens a synthetic placeholder pendency (TRM-645) -- either way the work order's own status recomputes to `AwaitingParts` (TRM-646). **Roles:** SysAdmin (any tenant); TenantOwner, Mechanic (own tenant only).", body = PurchaseOrderJson),
        (status = 400, description = "Bad request, including a non-positive quantity, an unknown part, a duplicate active order for this part on this work order, or a pendency named without its own work order", body = BadRequestErrorJson),
        (status = 404, description = "Part, work order, pendency or vehicle not found, **or any of them belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not raise purchase orders", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<PurchaseOrderJson>,
) -> HttpResponse<(StatusCode, Json<PurchaseOrderJson>)> {
    let part = resolve_part(&state, &locale, &current_user, payload.part_uuid).await?;
    if !can_create_purchase_order(&current_user, part.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::PurchaseOrderForbidden,
        ));
    }
    let work_order_id =
        resolve_work_order_id(&state, &locale, &current_user, payload.work_order_uuid).await?;
    let work_order_item_id =
        resolve_work_order_item_id(&state, &locale, payload.work_order_item_uuid).await?;
    let vehicle_id = resolve_vehicle_id(&state, &locale, &current_user, payload.vehicle_uuid).await?;

    let purchase_order = PurchaseOrder {
        id: None,
        uuid: None,
        tenant_id: part.tenant_id,
        part_id: part.id.unwrap_or_default(),
        quantity: payload.quantity,
        suggested_supplier: payload.suggested_supplier,
        observation: payload.observation,
        status: Default::default(),
        work_order_id,
        work_order_item_id,
        vehicle_id,
        ordered_at: None,
        expected_delivery_date: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };

    match use_case(&state).create(purchase_order).await {
        Ok(saved) => Ok((StatusCode::CREATED, Json(json(&state, saved).await))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "PurchaseOrder",
    path = "/purchase-order/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Purchase order UUID")),
    responses(
        (status = 200, description = "The purchase order. **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = PurchaseOrderJson),
        (status = 404, description = "Purchase order not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
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
) -> HttpResponse<Json<PurchaseOrderJson>> {
    let purchase_order = use_case(&state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::PurchaseOrderNotFound))?;
    if !can_read_purchase_order(&current_user, purchase_order.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::PurchaseOrderNotFound));
    }
    Ok(Json(json(&state, purchase_order).await))
}

#[utoipa::path(
    get,
    tag = "PurchaseOrder",
    path = "/purchase-order",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of the caller's own tenant's purchase orders (PD-028), most recently raised first. **Roles:** SysAdmin, TenantOwner, TenantUser, Driver, Mechanic (any authenticated role; SysAdmin sees every tenant, everyone else only their own).", body = PageJson<PurchaseOrderJson>),
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
) -> HttpResponse<Json<PageJson<PurchaseOrderJson>>> {
    let (page, page_size) = (page_query.page(), page_query.page_size());
    match use_case(&state).find_page(page, page_size).await {
        Ok((purchase_orders, total)) => {
            let mut rows = Vec::with_capacity(purchase_orders.len());
            for purchase_order in purchase_orders {
                rows.push(json(&state, purchase_order).await);
            }
            Ok(Json(PageJson::new(rows, page, page_size, total)))
        }
        Err(_) => Err(ExceptionResponse::InternalServerError(
            locale,
            ErrorKey::UnexpectedError,
        )),
    }
}

/// Shared by `mark_ordered`, `mark_purchased` and `cancel`: resolve the uuid,
/// check visibility (404) then administer rights (403).
async fn administered_purchase_order(
    state: &AppState,
    locale: &Locale,
    current_user: &User,
    uuid: String,
) -> Result<i64, ExceptionResponse> {
    let purchase_order = use_case(state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::PurchaseOrderNotFound))?;
    if !can_read_purchase_order(current_user, purchase_order.tenant_id) {
        return Err(ExceptionResponse::NotFound(
            locale.clone(),
            ErrorKey::PurchaseOrderNotFound,
        ));
    }
    if !can_administer_purchase_order(current_user, purchase_order.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale.clone(),
            ErrorKey::PurchaseOrderForbidden,
        ));
    }
    Ok(purchase_order.id.unwrap_or_default())
}

#[utoipa::path(
    put,
    tag = "PurchaseOrder",
    path = "/purchase-order/uuid/{uuid}/mark-ordered",
    params(("uuid" = String, Path, description = "Purchase order UUID")),
    request_body = MarkOrderedJson,
    responses(
        (status = 200, description = "`Requested` -> `Ordered` (TRM-641), recording when it was placed and, optionally, when it is expected (TRM-651). **Roles:** SysAdmin (any tenant); TenantOwner, Mechanic (own tenant only).", body = PurchaseOrderJson),
        (status = 400, description = "The order is not currently `Requested`", body = BadRequestErrorJson),
        (status = 404, description = "Purchase order not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not administer purchase orders", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn mark_ordered(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<MarkOrderedJson>,
) -> HttpResponse<Json<PurchaseOrderJson>> {
    let id = administered_purchase_order(&state, &locale, &current_user, uuid).await?;
    match use_case(&state)
        .mark_ordered(id, payload.ordered_at, payload.expected_delivery_date)
        .await
    {
        Ok(saved) => Ok(Json(json(&state, saved).await)),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    put,
    tag = "PurchaseOrder",
    path = "/purchase-order/uuid/{uuid}/mark-purchased",
    params(("uuid" = String, Path, description = "Purchase order UUID")),
    responses(
        (status = 200, description = "`Ordered` -> `Purchased` (TRM-641). If this order links a pendency, receiving it resolves a synthetic placeholder outright (TRM-647) or returns a real pendency to `Pending` (TRM-648), recomputing the work order's own status (TRM-649). **Roles:** SysAdmin (any tenant); TenantOwner, Mechanic (own tenant only).", body = PurchaseOrderJson),
        (status = 400, description = "The order is not currently `Ordered`", body = BadRequestErrorJson),
        (status = 404, description = "Purchase order not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not administer purchase orders", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn mark_purchased(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<PurchaseOrderJson>> {
    let id = administered_purchase_order(&state, &locale, &current_user, uuid).await?;
    match use_case(&state).mark_purchased(id).await {
        Ok(saved) => Ok(Json(json(&state, saved).await)),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    put,
    tag = "PurchaseOrder",
    path = "/purchase-order/uuid/{uuid}/cancel",
    params(("uuid" = String, Path, description = "Purchase order UUID")),
    responses(
        (status = 200, description = "Cancelled from any status (TRM-641); an already-`Cancelled` order is returned unchanged. **Roles:** SysAdmin (any tenant); TenantOwner, Mechanic (own tenant only).", body = PurchaseOrderJson),
        (status = 404, description = "Purchase order not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not administer purchase orders", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn cancel(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<PurchaseOrderJson>> {
    let id = administered_purchase_order(&state, &locale, &current_user, uuid).await?;
    match use_case(&state).cancel(id).await {
        Ok(saved) => Ok(Json(json(&state, saved).await)),
        Err(e) => Err(error(locale, &e.message)),
    }
}
