use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::customer_endpoint::find_visible;
use crate::endpoints::json::customer_day_off_json::CustomerDayOffJson;
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::can_administer_customer;
use business::domain::customer::Customer;
use business::domain::customer_day_off::CustomerDayOff;
use business::domain::user::User;
use business::gateway::customer_day_off_gateway::CustomerDayOffGateway;
use business::use_cases::customer_day_off_use_case::{
    CustomerDayOffUseCase, DAY_OFF_NOT_FOUND, DUPLICATE_DAY_OFF,
};

fn use_case(state: &AppState) -> CustomerDayOffUseCase {
    CustomerDayOffUseCase::new(CustomerDayOffGateway::new(state.conn.as_ref().clone()))
}

fn json(day_off: CustomerDayOff) -> CustomerDayOffJson {
    CustomerDayOffJson {
        uuid: day_off.uuid,
        date: day_off.date,
        reason: day_off.reason,
    }
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        DUPLICATE_DAY_OFF => ExceptionResponse::Conflict(locale, ErrorKey::DuplicateDayOff),
        DAY_OFF_NOT_FOUND => ExceptionResponse::NotFound(locale, ErrorKey::DayOffNotFound),
        _ => ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError),
    }
}

fn require_administrator(
    locale: &Locale,
    user: &User,
    customer: &Customer,
) -> Result<(), ExceptionResponse> {
    if can_administer_customer(user, customer.tenant_id) {
        Ok(())
    } else {
        Err(ExceptionResponse::Forbidden(
            locale.clone(),
            ErrorKey::CustomerForbidden,
        ))
    }
}

#[utoipa::path(
    post,
    tag = "Customer",
    path = "/customer/uuid/{uuid}/day-off",
    params(("uuid" = String, Path, description = "Customer UUID")),
    request_body = CustomerDayOffJson,
    responses(
        (status = 201, description = "The day off is recorded (HRMS-601). **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only).", body = CustomerDayOffJson),
        (status = 404, description = "Customer not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller may read this customer but not change it", body = ForbiddenErrorJson),
        (status = 409, description = "`DuplicateDayOff`: this customer already has a day off recorded for that date", body = BadRequestErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<CustomerDayOffJson>,
) -> HttpResponse<(StatusCode, Json<CustomerDayOffJson>)> {
    let customer = find_visible(&state, &locale, &current_user, uuid).await?;
    require_administrator(&locale, &current_user, &customer)?;
    match use_case(&state)
        .add(&customer, payload.date, payload.reason)
        .await
    {
        Ok(result) => Ok((StatusCode::CREATED, Json(json(result)))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "Customer",
    path = "/customer/uuid/{uuid}/day-offs",
    params(("uuid" = String, Path, description = "Customer UUID"), PageQuery),
    responses(
        (status = 200, description = "The customer's recorded days off, newest first (PD-028). **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = PageJson<CustomerDayOffJson>),
        (status = 404, description = "Customer not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
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
) -> HttpResponse<Json<PageJson<CustomerDayOffJson>>> {
    let customer = find_visible(&state, &locale, &current_user, uuid).await?;
    let (page, page_size) = (page_query.page(), page_query.page_size());
    match use_case(&state).history(&customer, page, page_size).await {
        Ok((items, total)) => Ok(Json(PageJson::new(
            items.into_iter().map(json).collect(),
            page,
            page_size,
            total,
        ))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    delete,
    tag = "Customer",
    path = "/customer/uuid/{uuid}/day-off/uuid/{day_off_uuid}",
    params(
        ("uuid" = String, Path, description = "Customer UUID"),
        ("day_off_uuid" = String, Path, description = "Day off UUID"),
    ),
    responses(
        (status = 204, description = "The day off is removed. **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only)."),
        (status = 404, description = "`CustomerNotFound` (also for another tenant's customer, PD-034) or `DayOffNotFound`", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller may read this customer but not change it", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn remove(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path((uuid, day_off_uuid)): Path<(String, String)>,
) -> HttpResponse<StatusCode> {
    let customer = find_visible(&state, &locale, &current_user, uuid).await?;
    require_administrator(&locale, &current_user, &customer)?;
    match use_case(&state).remove(&customer, day_off_uuid).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err(error(locale, &e.message)),
    }
}
