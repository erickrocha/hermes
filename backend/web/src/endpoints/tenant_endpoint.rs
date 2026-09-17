use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::business_plan_endpoint::response as business_plan_response;
use crate::endpoints::json::business_plan_json::BusinessPlanJson;
use crate::endpoints::json::tenant_json::{SetTenantPlanJson, TenantJson};
use crate::infrastructure::mapper::{Mapper, TenantMapper};
use axum::Json;
use axum::extract::{Extension, Path, State};
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
        (status = 404, description = "Tenant not found", body = NotFoundErrorJson),
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
        (status = 404, description = "Tenant not found", body = NotFoundErrorJson),
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
    responses(
        (status = 200, description = "List of tenants", body = Vec<TenantJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_all(
    state: State<AppState>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<Vec<TenantJson>>> {
    let use_case = TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()));
    if let Some(tenant_id) = current_user.tenant_id {
        return match use_case.find_by_id(tenant_id).await {
            Ok(tenant) => Ok(Json(vec![TenantMapper::json(tenant)])),
            Err(_) => Ok(Json(Vec::new())),
        };
    }
    if current_user.role != Role::SysAdmin {
        return Ok(Json(Vec::new()));
    }
    match use_case.find_all().await {
        Ok(tenants) => Ok(Json(TenantMapper::json_vec(tenants))),
        Err(_) => Ok(Json(Vec::new())),
    }
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
        (status = 404, description = "Tenant not found", body = NotFoundErrorJson),
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
    let domain = TenantMapper::domain(payload);
    let use_case = TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()));
    match use_case.update(id, domain).await {
        Ok(tenant) => Ok(Json(TenantMapper::json(tenant))),
        Err(err) => {
            if err.message.contains("not found") {
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

    let plan_use_case = BusinessPlanUseCase::new(BusinessPlanGateway::new(state.conn.as_ref().clone()));
    let plan = plan_use_case
        .find_by_id(payload.business_plan_id)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale, ErrorKey::BusinessPlanNotFound))?;

    let tenant_use_case = TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()));
    tenant_use_case
        .set_plan(id, payload.business_plan_id)
        .await
        .map_err(|_| ExceptionResponse::BadRequest(locale, ErrorKey::TenantUpdateFailed))?;

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
        (status = 404, description = "Tenant not found", body = NotFoundErrorJson),
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
