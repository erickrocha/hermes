use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::json::part_json::PartJson;
use crate::endpoints::json::stock_movement_json::{StockAdjustmentJson, StockMovementJson};
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::{can_create_part, can_create_stock_movement, can_read_part};
use business::domain::part::Part;
use business::domain::stock_movement::StockMovement;
use business::domain::user::User;
use business::gateway::part_gateway::PartGateway;
use business::gateway::stock_movement_gateway::StockMovementGateway;
use business::use_cases::part_use_case::{NAME_REQUIRED, NEGATIVE_MINIMUM_STOCK, PartUseCase, UNIT_REQUIRED};
use business::use_cases::stock_movement_use_case::{
    NON_NEGATIVE_BALANCE_REQUIRED, PART_NOT_FOUND as MOVEMENT_PART_NOT_FOUND, QUANTITY_MUST_BE_POSITIVE,
    StockMovementUseCase, VALUE_REQUIRED,
};

fn use_case(state: &AppState) -> PartUseCase<PartGateway> {
    PartUseCase::new(PartGateway::new(state.conn.as_ref().clone()))
}

fn movement_use_case(state: &AppState) -> StockMovementUseCase {
    let db = state.conn.as_ref().clone();
    StockMovementUseCase::new(StockMovementGateway::new(db.clone()), PartGateway::new(db))
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        MOVEMENT_PART_NOT_FOUND => ExceptionResponse::NotFound(locale, ErrorKey::PartNotFound),
        NAME_REQUIRED | UNIT_REQUIRED | NEGATIVE_MINIMUM_STOCK | QUANTITY_MUST_BE_POSITIVE
        | VALUE_REQUIRED | NON_NEGATIVE_BALANCE_REQUIRED => {
            ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)
        }
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

async fn json(state: &AppState, part: Part) -> PartJson {
    let current_stock = movement_use_case(state).current_stock(part.id.unwrap_or_default()).await.ok();
    PartJson {
        uuid: part.uuid,
        code: part.id.map(|id| format!("PC-{id}")),
        tenant_id: part.tenant_id,
        name: part.name,
        category: part.category,
        application: part.application,
        minimum_stock: part.minimum_stock,
        unit: part.unit,
        unit_value_cents: part.unit_value_cents,
        default_supplier: part.default_supplier,
        location: part.location,
        observation: part.observation,
        moving_average_cost_cents: part.moving_average_cost_cents,
        last_purchase_price_cents: part.last_purchase_price_cents,
        current_stock,
    }
}

fn movement_json(movement: StockMovement) -> StockMovementJson {
    StockMovementJson {
        uuid: movement.uuid,
        movement_type: Some(movement.movement_type.to_string()),
        quantity: movement.quantity,
        unit_value_cents: movement.unit_value_cents,
        total_value_cents: movement.total_value_cents,
        cost_source: Some(movement.cost_source.to_string()),
        supplier: movement.supplier,
        invoice_number: movement.invoice_number,
        entry_date: movement.entry_date,
    }
}

#[utoipa::path(
    post,
    tag = "Part",
    path = "/part",
    request_body = PartJson,
    responses(
        (status = 201, description = "The part is registered in this tenant's catalogue. `code` (`PC-{id}`) is server-derived, never taken from the caller (TRM-601). **Roles:** SysAdmin (unbound; names the owning tenant); TenantOwner, Mechanic (own tenant only).", body = PartJson),
        (status = 400, description = "Bad request, including a blank name, a blank unit, or a negative minimum stock", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not register parts", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<PartJson>,
) -> HttpResponse<(StatusCode, Json<PartJson>)> {
    if !can_create_part(&current_user, payload.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::PartForbidden));
    }

    let mut tenant_id = payload.tenant_id;
    if current_user.tenant_id.is_some() {
        tenant_id = current_user.tenant_id;
    }

    let part = Part {
        id: None,
        uuid: None,
        tenant_id,
        name: payload.name,
        category: payload.category,
        application: payload.application,
        minimum_stock: payload.minimum_stock,
        unit: payload.unit,
        unit_value_cents: payload.unit_value_cents,
        default_supplier: payload.default_supplier,
        location: payload.location,
        observation: payload.observation,
        moving_average_cost_cents: None,
        last_purchase_price_cents: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };

    match use_case(&state).create(part).await {
        Ok(saved) => Ok((StatusCode::CREATED, Json(json(&state, saved).await))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "Part",
    path = "/part/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Part UUID")),
    responses(
        (status = 200, description = "The part, with its current stock (TRM-602) summed live from the ledger. **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = PartJson),
        (status = 404, description = "Part not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
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
) -> HttpResponse<Json<PartJson>> {
    let part = use_case(&state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::PartNotFound))?;
    if !can_read_part(&current_user, part.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::PartNotFound));
    }
    Ok(Json(json(&state, part).await))
}

#[utoipa::path(
    get,
    tag = "Part",
    path = "/part",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of the caller's own tenant's parts (PD-028), by name. **Roles:** SysAdmin, TenantOwner, TenantUser, Driver, Mechanic (any authenticated role; SysAdmin sees every tenant, everyone else only their own).", body = PageJson<PartJson>),
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
) -> HttpResponse<Json<PageJson<PartJson>>> {
    let (page, page_size) = (page_query.page(), page_query.page_size());
    match use_case(&state).find_page(page, page_size).await {
        Ok((parts, total)) => {
            let mut rows = Vec::with_capacity(parts.len());
            for part in parts {
                rows.push(json(&state, part).await);
            }
            Ok(Json(PageJson::new(rows, page, page_size, total)))
        }
        Err(_) => Err(ExceptionResponse::InternalServerError(
            locale,
            ErrorKey::UnexpectedError,
        )),
    }
}

/// Both `add_entry` and `adjust` share this shape: resolve the uuid, check
/// visibility (404, hiding another tenant's part) then stores-write rights
/// (403).
async fn writable_part(
    state: &AppState,
    locale: &Locale,
    current_user: &User,
    uuid: String,
) -> Result<Part, ExceptionResponse> {
    let part = use_case(state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::PartNotFound))?;
    if !can_read_part(current_user, part.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale.clone(), ErrorKey::PartNotFound));
    }
    if !can_create_stock_movement(current_user, part.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale.clone(), ErrorKey::PartForbidden));
    }
    Ok(part)
}

#[utoipa::path(
    post,
    tag = "Part",
    path = "/part/uuid/{uuid}/entry",
    params(("uuid" = String, Path, description = "Part UUID")),
    request_body = StockMovementJson,
    responses(
        (status = 201, description = "The entry is recorded (TRM-611: either `unitValueCents` or `totalValueCents`, the other derived from `quantity`), and the part's moving average cost and last purchase price are updated in the same transaction (TRM-608/610). **Roles:** SysAdmin (any tenant); TenantOwner, Mechanic (own tenant only).", body = StockMovementJson),
        (status = 400, description = "Bad request, including a non-positive quantity or neither value given", body = BadRequestErrorJson),
        (status = 404, description = "Part not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not record stock movements", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add_entry(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<StockMovementJson>,
) -> HttpResponse<(StatusCode, Json<StockMovementJson>)> {
    let part = writable_part(&state, &locale, &current_user, uuid).await?;

    match movement_use_case(&state)
        .record_entry(
            part.id.unwrap_or_default(),
            payload.quantity,
            payload.unit_value_cents,
            payload.total_value_cents,
            payload.supplier,
            payload.invoice_number,
            payload.entry_date,
        )
        .await
    {
        Ok(saved) => Ok((StatusCode::CREATED, Json(movement_json(saved)))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    post,
    tag = "Part",
    path = "/part/uuid/{uuid}/adjustment",
    params(("uuid" = String, Path, description = "Part UUID")),
    request_body = StockAdjustmentJson,
    responses(
        (status = 201, description = "A single adjustment movement is recorded for the difference between the current ledger-derived stock and `newBalance` (TRM-604/617). **Roles:** SysAdmin (any tenant); TenantOwner, Mechanic (own tenant only).", body = StockMovementJson),
        (status = 400, description = "Bad request: `newBalance` must be finite and non-negative", body = BadRequestErrorJson),
        (status = 404, description = "Part not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not record stock movements", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn adjust(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<StockAdjustmentJson>,
) -> HttpResponse<(StatusCode, Json<StockMovementJson>)> {
    let part = writable_part(&state, &locale, &current_user, uuid).await?;

    match movement_use_case(&state)
        .adjust(part.id.unwrap_or_default(), payload.new_balance)
        .await
    {
        Ok(saved) => Ok((StatusCode::CREATED, Json(movement_json(saved)))),
        Err(e) => Err(error(locale, &e.message)),
    }
}
