use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::business_plan_endpoint::response as business_plan_response;
use crate::endpoints::json::business_plan_json::BusinessPlanJson;
use crate::endpoints::json::tenant_json::{SetTenantPlanJson, TenantJson};
use crate::infrastructure::mapper::{Mapper, TenantMapper};
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::{can_access_tenant, can_create_tenant, can_set_tenant_plan};
use business::domain::enums::Role;
use business::domain::user::User;
use business::gateway::business_plan_gateway::BusinessPlanGateway;
use business::gateway::tenant_gateway::TenantGateway;
use business::use_cases::business_plan_use_case::BusinessPlanUseCase;
use business::use_cases::tenant_use_case::TenantUseCase;

#[utoipa::path(
    post,
    tag = "Tenant",
    path = "/tenant",
    request_body = TenantJson,
    responses(
        (status = 201, description = "Tenant created", body = TenantJson),
        (status = 400, description = "Bad request", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<TenantJson>,
) -> HttpResponse<(StatusCode, Json<TenantJson>)> {
    if !can_create_tenant(&current_user) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::InvalidParameterValue,
        ));
    }
    let domain = TenantMapper::domain(payload);
    let use_case = TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()));
    match use_case.create(domain).await {
        Ok(tenant) => Ok((StatusCode::CREATED, Json(TenantMapper::json(tenant)))),
        Err(_) => Err(ExceptionResponse::BadRequest(
            locale,
            ErrorKey::TenantCreatedFailed,
        )),
    }
}

#[utoipa::path(
    get,
    tag = "Tenant",
    path = "/tenant/{id}",
    params(
        ("id" = i64, Path, description = "Tenant ID")
    ),
    responses(
        (status = 200, description = "Tenant found", body = TenantJson),
        (status = 404, description = "Tenant not found, **or it exists and belongs to another tenant**. PD-034/HRM-092: a tenant-bound caller is answered 404 rather than 403 on purpose, so that the existence of another customer's tenant is not disclosed. Do not treat this as a defect.", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_by_id(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(id): Path<i64>,
) -> HttpResponse<Json<TenantJson>> {
    if !can_access_tenant(&current_user, id) {
        return Err(ExceptionResponse::NotFound(
            locale,
            ErrorKey::TenantNotFound,
        ));
    }
    let use_case = TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()));
    match use_case.find_by_id(id).await {
        Ok(tenant) => Ok(Json(TenantMapper::json(tenant))),
        Err(_) => Err(ExceptionResponse::NotFound(
            locale,
            ErrorKey::TenantNotFound,
        )),
    }
}

#[utoipa::path(
    get,
    tag = "Tenant",
    path = "/tenant/uuid/{uuid}",
    params(
        ("uuid" = String, Path, description = "Tenant UUID")
    ),
    responses(
        (status = 200, description = "Tenant found", body = TenantJson),
        (status = 404, description = "Tenant not found, **or it exists and belongs to another tenant**. PD-034/HRM-092: a tenant-bound caller is answered 404 rather than 403 on purpose, so that the existence of another customer's tenant is not disclosed. Do not treat this as a defect.", body = NotFoundErrorJson),
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
) -> HttpResponse<Json<TenantJson>> {
    let use_case = TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()));
    match use_case.find_by_uuid(uuid).await {
        Ok(tenant) if can_access_tenant(&current_user, tenant.id.unwrap_or_default()) => {
            Ok(Json(TenantMapper::json(tenant)))
        }
        Ok(_) => Err(ExceptionResponse::NotFound(
            locale,
            ErrorKey::TenantNotFound,
        )),
        Err(_) => Err(ExceptionResponse::NotFound(
            locale,
            ErrorKey::TenantNotFound,
        )),
    }
}

#[utoipa::path(
    get,
    tag = "Tenant",
    path = "/tenant",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of tenants (PD-028)", body = PageJson<TenantJson>),
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
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<PageJson<TenantJson>>> {
    let use_case = TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()));
    let (page, page_size) = (page_query.page(), page_query.page_size());
    let search = page_query.search();

    // HRM-092: um caller preso a um tenant vê exatamente o seu, então a
    // "página" dele tem no máximo uma linha — paginar não muda isso.
    if let Some(tenant_id) = current_user.tenant_id {
        return match use_case.find_by_id(tenant_id).await {
            Ok(tenant) => Ok(Json(PageJson::new(vec![TenantMapper::json(tenant)], 0, page_size, 1))),
            Err(_) => Err(unreadable_tenants(locale)),
        };
    }
    if current_user.role != Role::SysAdmin {
        return Ok(Json(PageJson::new(Vec::new(), page, page_size, 0)));
    }
    match use_case.find_page(page, page_size, search.as_deref()).await {
        Ok((tenants, total)) => Ok(Json(PageJson::new(
            TenantMapper::json_vec(tenants),
            page,
            page_size,
            total,
        ))),
        Err(_) => Err(unreadable_tenants(locale)),
    }
}

/// DEF-XF-02: a failed query used to be answered with `200 {"items":[],
/// "totalItems":0}`, which reads to the console and to the operator exactly
/// like a platform that has no tenants at all. An empty page is a fact about
/// the data; it must never also be how a broken read looks.
fn unreadable_tenants(locale: Locale) -> ExceptionResponse {
    log::error!("[tenant_endpoint::list_all] Tenant query failed; answering 500 rather than an empty page");
    ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)
}

#[utoipa::path(
    put,
    tag = "Tenant",
    path = "/tenant/{id}",
    params(
        ("id" = i32, Path, description = "Tenant ID")
    ),
    request_body = TenantJson,
    responses(
        (status = 200, description = "Tenant updated", body = TenantJson),
        (status = 400, description = "Bad request", body = BadRequestErrorJson),
        (status = 404, description = "Tenant not found, **or it exists and belongs to another tenant**. PD-034/HRM-092: a tenant-bound caller is answered 404 rather than 403 on purpose, so that the existence of another customer's tenant is not disclosed. Do not treat this as a defect.", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn update(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(id): Path<i64>,
    Json(payload): Json<TenantJson>,
) -> HttpResponse<Json<TenantJson>> {
    if !can_access_tenant(&current_user, id) {
        return Err(ExceptionResponse::NotFound(
            locale,
            ErrorKey::TenantNotFound,
        ));
    }
    update_tenant(&state, locale, id, payload).await
}

#[utoipa::path(
    put,
    tag = "Tenant",
    path = "/tenant/uuid/{uuid}",
    params(
        ("uuid" = String, Path, description = "Tenant UUID")
    ),
    request_body = TenantJson,
    responses(
        (status = 200, description = "Tenant updated", body = TenantJson),
        (status = 400, description = "Bad request", body = BadRequestErrorJson),
        (status = 404, description = "Tenant not found, **or it exists and belongs to another tenant**. PD-034/HRM-092: a tenant-bound caller is answered 404 rather than 403 on purpose, so that the existence of another customer's tenant is not disclosed. Do not treat this as a defect.", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn update_by_uuid(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<TenantJson>,
) -> HttpResponse<Json<TenantJson>> {
    let id = resolve_uuid(&state, &locale, &current_user, uuid).await?;
    update_tenant(&state, locale, id, payload).await
}

/// HRMS-204/AD-010: the UUID is the tenant's public identifier, so every
/// operation available by internal id is available by UUID too (DEF-TP-04).
/// The `/{id}` routes stay for callers that already hold an id; a console can
/// now work entirely in UUIDs and never put a sequential id — and therefore
/// the number of customers — in a URL.
async fn resolve_uuid(
    state: &AppState,
    locale: &Locale,
    current_user: &User,
    uuid: String,
) -> Result<i64, ExceptionResponse> {
    let use_case = TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()));
    let tenant = use_case
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::TenantNotFound))?;
    let id = tenant.id.unwrap_or_default();
    // PD-034/HRM-092: a tenant that is not yours is answered 404, exactly as
    // on the `/{id}` routes -- the public identifier must not become a way to
    // tell "does not exist" from "not yours".
    if !can_access_tenant(current_user, id) {
        return Err(ExceptionResponse::NotFound(locale.clone(), ErrorKey::TenantNotFound));
    }
    Ok(id)
}

async fn update_tenant(
    state: &AppState,
    locale: Locale,
    id: i64,
    payload: TenantJson,
) -> HttpResponse<Json<TenantJson>> {
    let domain = TenantMapper::domain(payload);
    let use_case = TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()));
    match use_case.update(id, domain).await {
        Ok(tenant) => Ok(Json(TenantMapper::json(tenant))),
        Err(err) => {
            if err.is_not_found() {
                Err(ExceptionResponse::NotFound(
                    locale,
                    ErrorKey::TenantNotFound,
                ))
            } else {
                Err(ExceptionResponse::BadRequest(
                    locale,
                    ErrorKey::TenantUpdateFailed,
                ))
            }
        }
    }
}

#[utoipa::path(
    post,
    tag = "Tenant",
    path = "/tenant/{id}/plan",
    params(
        ("id" = i32, Path, description = "Tenant ID")
    ),
    request_body = SetTenantPlanJson,
    responses(
        (status = 200, description = "Tenant plan set", body = BusinessPlanJson),
        (status = 400, description = "Bad request", body = BadRequestErrorJson),
        (status = 404, description = "Tenant or business plan not found", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add_plan(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(id): Path<i64>,
    Json(payload): Json<SetTenantPlanJson>,
) -> HttpResponse<Json<BusinessPlanJson>> {
    // Only an unbound platform administrator may set a tenant's plan (HRMS-224).
    if !can_set_tenant_plan(&current_user) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::InvalidParameterValue,
        ));
    }
    set_tenant_plan(&state, locale, id, payload).await
}

#[utoipa::path(
    post,
    tag = "Tenant",
    path = "/tenant/uuid/{uuid}/plan",
    params(
        ("uuid" = String, Path, description = "Tenant UUID")
    ),
    request_body = SetTenantPlanJson,
    responses(
        (status = 200, description = "Tenant plan set", body = BusinessPlanJson),
        (status = 400, description = "Bad request", body = BadRequestErrorJson),
        (status = 404, description = "Tenant or business plan not found", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add_plan_by_uuid(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<SetTenantPlanJson>,
) -> HttpResponse<Json<BusinessPlanJson>> {
    if !can_set_tenant_plan(&current_user) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::InvalidParameterValue,
        ));
    }
    let id = resolve_uuid(&state, &locale, &current_user, uuid).await?;
    set_tenant_plan(&state, locale, id, payload).await
}

async fn set_tenant_plan(
    state: &AppState,
    locale: Locale,
    id: i64,
    payload: SetTenantPlanJson,
) -> HttpResponse<Json<BusinessPlanJson>> {
    let plan_use_case = BusinessPlanUseCase::new(BusinessPlanGateway::new(state.conn.as_ref().clone()));
    let plan = plan_use_case
        .find_by_id(payload.business_plan_id)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::BusinessPlanNotFound))?;

    let tenant_use_case = TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()));
    tenant_use_case
        .set_plan(id, payload.business_plan_id)
        .await
        // DEF-TP-05: an unknown tenant is a 404, as the documented response
        // says and as an unknown plan already answered — not a 400.
        .map_err(|err| {
            if err.is_not_found() {
                ExceptionResponse::NotFound(locale, ErrorKey::TenantNotFound)
            } else {
                ExceptionResponse::BadRequest(locale, ErrorKey::TenantUpdateFailed)
            }
        })?;

    Ok(Json(business_plan_response(plan)))
}

#[utoipa::path(
    get,
    tag = "Tenant",
    path = "/tenant/{id}/plan",
    params(
        ("id" = i32, Path, description = "Tenant ID")
    ),
    responses(
        (status = 200, description = "Tenant's current plan, if one is set", body = Option<BusinessPlanJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 404, description = "Tenant not found, **or it exists and belongs to another tenant**. PD-034/HRM-092: a tenant-bound caller is answered 404 rather than 403 on purpose, so that the existence of another customer's tenant is not disclosed. Do not treat this as a defect.", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_active_plan(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(id): Path<i64>,
) -> HttpResponse<Json<Option<BusinessPlanJson>>> {
    if !can_access_tenant(&current_user, id) {
        return Err(ExceptionResponse::NotFound(
            locale,
            ErrorKey::TenantNotFound,
        ));
    }
    active_plan(&state, locale, id).await
}

#[utoipa::path(
    get,
    tag = "Tenant",
    path = "/tenant/uuid/{uuid}/plan",
    params(
        ("uuid" = String, Path, description = "Tenant UUID")
    ),
    responses(
        (status = 200, description = "Tenant's current plan, if one is set", body = Option<BusinessPlanJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 404, description = "Tenant not found, **or it exists and belongs to another tenant**. PD-034/HRM-092: a tenant-bound caller is answered 404 rather than 403 on purpose, so that the existence of another customer's tenant is not disclosed. Do not treat this as a defect.", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_active_plan_by_uuid(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<Option<BusinessPlanJson>>> {
    let id = resolve_uuid(&state, &locale, &current_user, uuid).await?;
    active_plan(&state, locale, id).await
}

async fn active_plan(
    state: &AppState,
    locale: Locale,
    id: i64,
) -> HttpResponse<Json<Option<BusinessPlanJson>>> {
    let tenant_use_case = TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()));
    let tenant = tenant_use_case
        .find_by_id(id)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale, ErrorKey::TenantNotFound))?;

    let Some(business_plan_id) = tenant.business_plan_id else {
        return Ok(Json(None));
    };

    let plan_use_case = BusinessPlanUseCase::new(BusinessPlanGateway::new(state.conn.as_ref().clone()));
    match plan_use_case.find_by_id(business_plan_id).await {
        Ok(plan) => Ok(Json(Some(business_plan_response(plan)))),
        Err(_) => Ok(Json(None)),
    }
}
