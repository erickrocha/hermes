use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::json::vehicle_expense_json::VehicleExpenseJson;
use crate::endpoints::vehicle_endpoint::find_visible;
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::commons::functions::bytes_para_string;
use business::commons::gateway::Gateway;
use business::domain::authorization::{can_create_vehicle_expense, can_read_vehicle_expense};
use business::domain::enums::ExpenseOrigin;
use business::domain::user::User;
use business::domain::vehicle_expense::VehicleExpense;
use business::gateway::vehicle_expense_gateway::VehicleExpenseGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::use_cases::vehicle_expense_use_case::{
    CATEGORY_REQUIRED, COMPETENCE_PERIOD_REQUIRED, DUPLICATE_EXPENSE_INVOICE,
    VALUE_MUST_BE_POSITIVE, VEHICLE_NOT_FOUND, VehicleExpenseUseCase,
};

fn use_case(state: &AppState) -> VehicleExpenseUseCase {
    let db = state.conn.as_ref().clone();
    VehicleExpenseUseCase::new(
        VehicleExpenseGateway::new(db.clone()),
        VehicleGateway::new(db),
    )
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        VEHICLE_NOT_FOUND => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
        DUPLICATE_EXPENSE_INVOICE => {
            ExceptionResponse::BadRequest(locale, ErrorKey::DuplicateExpenseInvoice)
        }
        CATEGORY_REQUIRED | COMPETENCE_PERIOD_REQUIRED | VALUE_MUST_BE_POSITIVE => {
            ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)
        }
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

async fn json(state: &AppState, expense: VehicleExpense) -> VehicleExpenseJson {
    let vehicle_uuid = VehicleGateway::new(state.conn.as_ref().clone())
        .find_by_id(expense.vehicle_id)
        .await
        .ok()
        .flatten()
        .map(|m| bytes_para_string(m.uuid))
        .unwrap_or_default();

    VehicleExpenseJson {
        uuid: expense.uuid,
        tenant_id: expense.tenant_id,
        vehicle_uuid,
        category: expense.category,
        competence_period: expense.competence_period,
        issue_date: expense.issue_date,
        supplier: expense.supplier,
        invoice_number: expense.invoice_number,
        description: expense.description,
        value_cents: expense.value_cents,
        origin: Some(expense.origin.to_string()),
    }
}

#[utoipa::path(
    post,
    tag = "VehicleExpense",
    path = "/vehicle-expense",
    request_body = VehicleExpenseJson,
    responses(
        (status = 201, description = "The vehicle expense is recorded (TRM-660). `origin` is server-derived (`Manual`), never taken from the caller. **Roles:** SysAdmin (unbound; names the owning tenant); TenantOwner (own tenant only).", body = VehicleExpenseJson),
        (status = 400, description = "Bad request, including a blank category, a blank competence period, a non-positive value, an unknown vehicle, or a duplicate invoice number in the same category for the same vehicle (TRM-661)", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not record vehicle expenses", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<VehicleExpenseJson>,
) -> HttpResponse<(StatusCode, Json<VehicleExpenseJson>)> {
    if !can_create_vehicle_expense(&current_user, payload.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::VehicleExpenseForbidden,
        ));
    }

    let mut tenant_id = payload.tenant_id;
    if current_user.tenant_id.is_some() {
        tenant_id = current_user.tenant_id;
    }

    let vehicle = find_visible(&state, &locale, &current_user, payload.vehicle_uuid).await?;

    let expense = VehicleExpense {
        id: None,
        uuid: None,
        tenant_id,
        vehicle_id: vehicle.id.unwrap_or_default(),
        category: payload.category,
        competence_period: payload.competence_period,
        issue_date: payload.issue_date,
        supplier: payload.supplier,
        invoice_number: payload.invoice_number,
        description: payload.description,
        value_cents: payload.value_cents,
        origin: ExpenseOrigin::Manual,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };

    match use_case(&state).create(expense).await {
        Ok(saved) => Ok((StatusCode::CREATED, Json(json(&state, saved).await))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "VehicleExpense",
    path = "/vehicle-expense/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Vehicle expense UUID")),
    responses(
        (status = 200, description = "The vehicle expense. **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = VehicleExpenseJson),
        (status = 404, description = "Vehicle expense not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
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
) -> HttpResponse<Json<VehicleExpenseJson>> {
    let expense = use_case(&state).find_by_uuid(uuid).await.map_err(|_| {
        ExceptionResponse::NotFound(locale.clone(), ErrorKey::VehicleExpenseNotFound)
    })?;
    if !can_read_vehicle_expense(&current_user, expense.tenant_id) {
        return Err(ExceptionResponse::NotFound(
            locale,
            ErrorKey::VehicleExpenseNotFound,
        ));
    }
    Ok(Json(json(&state, expense).await))
}

#[utoipa::path(
    get,
    tag = "VehicleExpense",
    path = "/vehicle-expense",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of the caller's own tenant's vehicle expenses (PD-028), most recent issue date first. **Roles:** SysAdmin, TenantOwner, TenantUser, Driver, Mechanic (any authenticated role; SysAdmin sees every tenant, everyone else only their own).", body = PageJson<VehicleExpenseJson>),
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
) -> HttpResponse<Json<PageJson<VehicleExpenseJson>>> {
    let (page, page_size) = (page_query.page(), page_query.page_size());
    match use_case(&state).find_page(page, page_size).await {
        Ok((expenses, total)) => {
            let mut rows = Vec::with_capacity(expenses.len());
            for expense in expenses {
                rows.push(json(&state, expense).await);
            }
            Ok(Json(PageJson::new(rows, page, page_size, total)))
        }
        Err(_) => Err(ExceptionResponse::InternalServerError(
            locale,
            ErrorKey::UnexpectedError,
        )),
    }
}
