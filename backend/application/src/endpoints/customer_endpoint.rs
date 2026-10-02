use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::customer_json::CustomerJson;
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::infrastructure::mapper::{CustomerMapper, Mapper};
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::{
    can_administer_customer, can_create_customer, can_read_customer,
};
use business::domain::customer::Customer;
use business::domain::user::User;
use business::gateway::customer_gateway::CustomerGateway;
use business::use_cases::customer_use_case::CustomerUseCase;

fn use_case(state: &AppState) -> CustomerUseCase {
    CustomerUseCase::new(CustomerGateway::new(state.conn.as_ref().clone()))
}

#[utoipa::path(
    post,
    tag = "Customer",
    path = "/customer",
    request_body = CustomerJson,
    responses(
        (status = 201, description = "Customer registered. **Roles:** SysAdmin (unbound; names the owning tenant); TenantOwner (own tenant only).", body = CustomerJson),
        (status = 400, description = "Bad request", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not register customers", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<CustomerJson>,
) -> HttpResponse<(StatusCode, Json<CustomerJson>)> {
    let mut domain = CustomerMapper::domain(payload);

    if !can_create_customer(&current_user, domain.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::CustomerForbidden,
        ));
    }

    // HRMS-921's rule, applied to customers: for a tenant-bound caller the
    // owning tenant is the caller's own, never the payload's.
    if current_user.tenant_id.is_some() {
        domain.tenant_id = current_user.tenant_id;
    }

    match use_case(&state).create(domain).await {
        Ok(customer) => Ok((StatusCode::CREATED, Json(CustomerMapper::json(customer)))),
        Err(_) => Err(ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)),
    }
}

#[utoipa::path(
    get,
    tag = "Customer",
    path = "/customer",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of the caller's own customers (PD-028). **Roles:** SysAdmin, TenantOwner, TenantUser, Driver, Mechanic (any authenticated role; SysAdmin sees every tenant, everyone else only their own).", body = PageJson<CustomerJson>),
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
) -> HttpResponse<Json<PageJson<CustomerJson>>> {
    let (page, page_size) = (page_query.page(), page_query.page_size());
    let search = page_query.search();

    match use_case(&state)
        .find_page(page, page_size, search.as_deref())
        .await
    {
        Ok((customers, total)) => Ok(Json(PageJson::new(
            CustomerMapper::json_vec(customers),
            page,
            page_size,
            total,
        ))),
        Err(_) => Err(ExceptionResponse::InternalServerError(
            locale,
            ErrorKey::UnexpectedError,
        )),
    }
}

#[utoipa::path(
    get,
    tag = "Customer",
    path = "/customer/uuid/{uuid}",
    params(
        ("uuid" = String, Path, description = "Customer UUID")
    ),
    responses(
        (status = 200, description = "Customer found. **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = CustomerJson),
        (status = 404, description = "Customer not found, **or it exists and belongs to another tenant** (PD-034): a tenant-bound caller is answered 404 rather than 403 on purpose, so the existence of another customer's data is not disclosed.", body = NotFoundErrorJson),
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
) -> HttpResponse<Json<CustomerJson>> {
    let customer = find_visible(&state, &locale, &current_user, uuid).await?;
    Ok(Json(CustomerMapper::json(customer)))
}

#[utoipa::path(
    put,
    tag = "Customer",
    path = "/customer/uuid/{uuid}",
    params(
        ("uuid" = String, Path, description = "Customer UUID")
    ),
    request_body = CustomerJson,
    responses(
        (status = 200, description = "Customer updated. **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only).", body = CustomerJson),
        (status = 400, description = "Bad request", body = BadRequestErrorJson),
        (status = 404, description = "Customer not found, **or it exists and belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller may read this customer but not change it", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn update(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<CustomerJson>,
) -> HttpResponse<Json<CustomerJson>> {
    let existing = find_visible(&state, &locale, &current_user, uuid).await?;

    if !can_administer_customer(&current_user, existing.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::CustomerForbidden,
        ));
    }

    let id = existing.id.unwrap_or_default();
    let domain = CustomerMapper::domain(payload);

    match use_case(&state).update(id, domain).await {
        Ok(customer) => Ok(Json(CustomerMapper::json(customer))),
        Err(_) => Err(ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)),
    }
}

/// `PD-034`: the same shape `vehicle_endpoint::find_visible` uses, in one
/// place so no handler can state it differently -- a customer outside the
/// caller's boundary is **not found**, not forbidden.
pub(crate) async fn find_visible(
    state: &AppState,
    locale: &Locale,
    current_user: &User,
    uuid: String,
) -> Result<Customer, ExceptionResponse> {
    let customer = use_case(state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::CustomerNotFound))?;

    if !can_read_customer(current_user, customer.tenant_id) {
        return Err(ExceptionResponse::NotFound(
            locale.clone(),
            ErrorKey::CustomerNotFound,
        ));
    }

    Ok(customer)
}
