use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::change_password_request::ChangePasswordRequest;
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::user_json::UserJson;
use crate::infrastructure::mapper::{Mapper, UserMapper};
use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use business::domain::enums::Role;
use business::domain::user::User;
use business::gateway::user_gateway::UserGateway;
use business::use_cases::user_use_case::UserUseCase;

#[utoipa::path(
    post,
    tag = "User",
    path = "/user",
    request_body = UserJson,
    responses(
        (status = 201, description = "User created", body = UserJson),
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
    Json(payload): Json<UserJson>,
) -> HttpResponse<(StatusCode, Json<UserJson>)> {
    let mut domain = UserMapper::domain(payload);

    match current_user.role {
        Role::SysAdmin => {
            if domain.role == Role::SysAdmin {
                domain.tenant_id = None;
                domain.first_login = false;
            } else if domain.role == Role::TenantOwner && domain.tenant_id.is_some() {
                domain.first_login = true;
            } else {
                return Err(ExceptionResponse::BadRequest(
                    locale,
                    ErrorKey::InvalidParameterValue,
                ));
            }
        }
        Role::TenantOwner => {
            // TenantOwner can only create users for their own tenant
            if let Some(tenant_id) = current_user.tenant_id {
                domain.tenant_id = Some(tenant_id);
                domain.role = Role::TenantOwner;
                domain.first_login = true;
            } else {
                return Err(ExceptionResponse::Forbidden(
                    locale,
                    ErrorKey::RequiredHeaderValueMissing,
                ));
            }
        }
        _ => {
            return Err(ExceptionResponse::Forbidden(
                locale,
                ErrorKey::RequiredHeaderValueMissing,
            ));
        }
    }

    let use_case = UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()));
    match use_case.create(domain).await {
        Ok(user) => Ok((StatusCode::CREATED, Json(UserMapper::json(user)))),
        Err(_) => Err(ExceptionResponse::BadRequest(
            locale,
            ErrorKey::RequiredParameterMissing,
        )),
    }
}

#[utoipa::path(
    get,
    tag = "User",
    path = "/user",
    responses(
        (status = 200, description = "List of users", body = Vec<UserJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_all(
    state: State<AppState>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<Vec<UserJson>>> {
    let use_case = UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()));

    let users_result = match current_user.role {
        Role::SysAdmin => use_case.find_all().await,
        Role::TenantOwner => {
            if let Some(tenant_id) = current_user.tenant_id {
                use_case.find_all_by_tenant_id(tenant_id).await
            } else {
                Ok(Vec::new())
            }
        }
        _ => Ok(Vec::new()),
    };

    match users_result {
        Ok(users) => Ok(Json(UserMapper::json_vec(users))),
        Err(_) => Ok(Json(Vec::new())),
    }
}

#[utoipa::path(
    get,
    tag = "User",
    path = "/user/{id}",
    params(
        ("id" = i32, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User found", body = UserJson),
        (status = 404, description = "User not found", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_by_id(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(id): Path<i64>,
) -> HttpResponse<Json<UserJson>> {
    let use_case = UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()));
    match use_case.find_by_id(id).await {
        Ok(user) => {
            if current_user.role == Role::TenantOwner && user.tenant_id != current_user.tenant_id {
                return Err(ExceptionResponse::Forbidden(
                    locale,
                    ErrorKey::RequiredHeaderValueMissing,
                ));
            }
            Ok(Json(UserMapper::json(user)))
        }
        Err(_) => Err(ExceptionResponse::NotFound(
            locale,
            ErrorKey::RequiredParameterMissing,
        )),
    }
}

#[utoipa::path(
    put,
    tag = "User",
    path = "/user/{id}",
    params(
        ("id" = i32, Path, description = "User ID")
    ),
    request_body = UserJson,
    responses(
        (status = 200, description = "User updated", body = UserJson),
        (status = 400, description = "Bad request", body = BadRequestErrorJson),
        (status = 404, description = "User not found", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn update(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(id): Path<i64>,
    Json(payload): Json<UserJson>,
) -> HttpResponse<Json<UserJson>> {
    let mut domain = UserMapper::domain(payload);

    let use_case = UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()));

    if current_user.id == Some(id) && !domain.enabled {
        return Err(ExceptionResponse::BadRequest(
            locale,
            ErrorKey::InvalidParameterValue,
        ));
    }

    // Check permissions: SysAdmin or self-update allowed, otherwise TenantOwner restricted to same tenant
    if current_user.role != Role::SysAdmin && current_user.id != Some(id) {
        if current_user.role == Role::TenantOwner {
            let existing = use_case.find_by_id(id).await.map_err(|_| {
                ExceptionResponse::NotFound(locale, ErrorKey::RequiredParameterMissing)
            })?;
            if existing.tenant_id != current_user.tenant_id {
                return Err(ExceptionResponse::Forbidden(
                    locale,
                    ErrorKey::RequiredHeaderValueMissing,
                ));
            }
            domain.tenant_id = current_user.tenant_id;
            domain.role = Role::TenantOwner;
        } else {
            return Err(ExceptionResponse::Forbidden(
                locale,
                ErrorKey::RequiredHeaderValueMissing,
            ));
        }
    }

    if current_user.role == Role::SysAdmin {
        if domain.role == Role::SysAdmin {
            domain.tenant_id = None;
        } else if domain.role != Role::TenantOwner || domain.tenant_id.is_none() {
            return Err(ExceptionResponse::BadRequest(
                locale,
                ErrorKey::InvalidParameterValue,
            ));
        }
    } else if current_user.role == Role::TenantOwner {
        domain.tenant_id = current_user.tenant_id;
        domain.role = Role::TenantOwner;
    }

    match use_case.update(id, domain).await {
        Ok(user) => Ok(Json(UserMapper::json(user))),
        Err(_) => Err(ExceptionResponse::BadRequest(
            locale,
            ErrorKey::RequiredParameterMissing,
        )),
    }
}

#[utoipa::path(
    put,
    tag = "User",
    path = "/user/change-password",
    request_body = ChangePasswordRequest,
    responses(
        (status = 200, description = "Password changed"),
        (status = 400, description = "Bad request", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn change_password(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<ChangePasswordRequest>,
) -> HttpResponse<StatusCode> {
    let use_case = UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()));

    let user_id = current_user.id.unwrap();

    match use_case
        .change_password(user_id, payload.current_password, payload.new_password)
        .await
    {
        Ok(_) => Ok(StatusCode::OK),
        Err(e) if e.message == "Current password is incorrect" => Err(
            ExceptionResponse::BadRequest(locale, ErrorKey::InvalidCurrentPassword),
        ),
        Err(_) => Err(ExceptionResponse::BadRequest(
            locale,
            ErrorKey::RequiredParameterMissing,
        )),
    }
}
