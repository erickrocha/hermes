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
use business::domain::authorization::{
    can_administer_user, can_create_user_with_role, can_reassign_role, CreationTenant,
};
use business::domain::enums::Role;
use business::domain::user::User;
use business::gateway::tenant_gateway::TenantGateway;
use business::gateway::user_gateway::UserGateway;
use business::use_cases::tenant_use_case::TenantUseCase;
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

    // PD-019's creation hierarchy (EPIC-IA-04): an unbound platform
    // administrator creates administrators and tenant owners; a tenant
    // owner creates tenant users in their own tenant only. See
    // business::domain::authorization for why this is not the rule the
    // code used to implement.
    let Some(creation_tenant) = can_create_user_with_role(&current_user, &domain.role) else {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::RequiredHeaderValueMissing,
        ));
    };

    match creation_tenant {
        CreationTenant::Fixed(tenant_id) => {
            domain.tenant_id = tenant_id;
            // A new platform administrator (no tenant) needs no invitation
            // step; a tenant user created into an existing tenant does
            // (HRMS-120).
            domain.first_login = tenant_id.is_some();
        }
        CreationTenant::CallerChosen => {
            let Some(tenant_id) = domain.tenant_id else {
                return Err(ExceptionResponse::BadRequest(
                    locale,
                    ErrorKey::InvalidParameterValue,
                ));
            };
            let tenant_use_case = TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()));
            if tenant_use_case.find_by_id(tenant_id).await.is_err() {
                return Err(ExceptionResponse::BadRequest(
                    locale,
                    ErrorKey::InvalidParameterValue,
                ));
            }
            domain.tenant_id = Some(tenant_id);
            domain.first_login = true;
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
    let user = use_case
        .find_by_id(id)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale, ErrorKey::RequiredParameterMissing))?;

    // EPIC-IA-05-S01/HRMS-117: a tenant user has no user-administration
    // rights at all -- this used to check only whether a TenantOwner's
    // tenant matched, leaving a TenantUser (or a TenantOwner probing a
    // foreign tenant) free to read any account by id.
    if !can_administer_user(&current_user, user.id, user.tenant_id) {
        return Err(ExceptionResponse::NotFound(
            locale,
            ErrorKey::RequiredParameterMissing,
        ));
    }

    Ok(Json(UserMapper::json(user)))
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

    let existing = use_case
        .find_by_id(id)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale, ErrorKey::RequiredParameterMissing))?;

    // EPIC-IA-05-S01/HRMS-118: self, an unbound platform administrator, or
    // the tenant owner of that exact tenant -- nobody else may touch this
    // record at all (a tenant user editing anyone, including themselves via
    // this route rather than change-password, stops here too).
    //
    // Not found rather than forbidden (EPIC-IA-05-S04/HRMS-122's shape,
    // stated for tenant records and extended here to user records for the
    // same reason: a caller outside the boundary should not learn that the
    // id exists at all).
    if !can_administer_user(&current_user, existing.id, existing.tenant_id) {
        return Err(ExceptionResponse::NotFound(
            locale,
            ErrorKey::RequiredParameterMissing,
        ));
    }

    // PD-019: reassigning the hierarchy is an unbound platform
    // administrator's decision alone. Everyone else editing an account --
    // including that account's own owner -- keeps its existing role and
    // tenant; only the other fields in the payload take effect.
    if can_reassign_role(&current_user, &domain.role, domain.tenant_id) {
        if domain.role == Role::TenantOwner {
            let tenant_id = domain.tenant_id.expect("can_reassign_role requires Some for TenantOwner");
            let tenant_use_case = TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()));
            if tenant_use_case.find_by_id(tenant_id).await.is_err() {
                return Err(ExceptionResponse::BadRequest(
                    locale,
                    ErrorKey::InvalidParameterValue,
                ));
            }
        }
    } else {
        domain.role = existing.role.clone();
        domain.tenant_id = existing.tenant_id;
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
