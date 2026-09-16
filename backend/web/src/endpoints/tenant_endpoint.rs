use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::tenant_json::TenantJson;
use crate::endpoints::json::tenant_plan_json::TenantPlanJson;
use crate::infrastructure::mapper::{Mapper, TenantMapper, TenantPlanMapper};
use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use business::domain::enums::Role;
use business::domain::user::User;
use business::gateway::tenant_gateway::TenantGateway;
use business::gateway::tenant_plan_gateway::TenantPlanGateway;
use business::use_cases::tenant_plan_use_case::TenantPlanUseCase;
use business::use_cases::tenant_use_case::TenantUseCase;

fn can_access_tenant(user: &User, tenant_id: i64) -> bool {
    user.tenant_id == Some(tenant_id) || (user.role == Role::SysAdmin && user.tenant_id.is_none())
}

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
    if current_user.role != Role::SysAdmin || current_user.tenant_id.is_some() {
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
    request_body = TenantPlanJson,
    responses(
        (status = 200, description = "Tenant plan updated", body = TenantPlanJson),
        (status = 400, description = "Bad request", body = BadRequestErrorJson),
        (status = 404, description = "Tenant not found", body = NotFoundErrorJson),
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
    Json(payload): Json<TenantPlanJson>,
) -> HttpResponse<Json<TenantPlanJson>> {
    if current_user.role != Role::SysAdmin || current_user.tenant_id.is_some() {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::InvalidParameterValue,
        ));
    }

    let mut domain = TenantPlanMapper::domain(payload);
    // Ensure the tenant ID matches the path parameter
    domain.tenant_id = id;

    let use_case = TenantPlanUseCase::new(TenantPlanGateway::new(state.conn.as_ref().clone()));
    match use_case.create_or_update(domain).await {
        Ok(tenant_plan) => Ok(Json(TenantPlanMapper::json(tenant_plan))),
        Err(_) => Err(ExceptionResponse::BadRequest(
            locale,
            ErrorKey::TenantUpdateFailed,
        )),
    }
}

#[utoipa::path(
    get,
    tag = "Tenant",
    path = "/tenant/{id}/plan",
    params(
        ("id" = i32, Path, description = "Tenant ID")
    ),
    responses(
        (status = 200, description = "Tenant active plan found", body = Option<TenantPlanJson>),
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
) -> HttpResponse<Json<Option<TenantPlanJson>>> {
    if !can_access_tenant(&current_user, id) {
        return Err(ExceptionResponse::NotFound(
            locale,
            ErrorKey::TenantNotFound,
        ));
    }

    let use_case = TenantPlanUseCase::new(TenantPlanGateway::new(state.conn.as_ref().clone()));
    match use_case.find_active_by_tenant_id(id).await {
        Ok(plan_opt) => Ok(Json(plan_opt.map(TenantPlanMapper::json))),
        Err(_) => Err(ExceptionResponse::BadRequest(
            locale,
            ErrorKey::TenantNotFound,
        )),
    }
}
