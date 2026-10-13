use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::access_profile_json::{
    AccessProfileJson, AssignProfileJson, EffectivePermissionsJson, PermissionJson,
};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson, UnauthorizedErrorJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::access_profile::AccessProfile;
use business::domain::authorization::can_manage_access_profiles;
use business::domain::enums::Role;
use business::domain::permission::GRANTABLE;
use business::domain::user::User;
use business::gateway::access_profile_gateway::AccessProfileGateway;
use business::gateway::user_gateway::{UserGateway, UserLookupStore};
use business::use_cases::access_profile_use_case::{AccessProfileUseCase, DUPLICATE_NAME, PROFILE_DOES_NOT_FIT, PROFILE_IN_USE};
use std::str::FromStr;

fn use_case(state: &AppState) -> AccessProfileUseCase<AccessProfileGateway> {
    AccessProfileUseCase::new(AccessProfileGateway::new(state.conn.as_ref().clone()))
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        DUPLICATE_NAME => ExceptionResponse::Conflict(locale, ErrorKey::DuplicateAccessProfile),
        PROFILE_IN_USE | PROFILE_DOES_NOT_FIT => ExceptionResponse::Conflict(locale, ErrorKey::InvalidParameterValue),
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

fn json(p: AccessProfile) -> AccessProfileJson {
    AccessProfileJson {
        uuid: p.uuid,
        tenant_id: p.tenant_id,
        name: p.name,
        base_role: p.base_role.to_string(),
        active: p.active,
        permissions: p.permissions,
    }
}

fn domain(
    locale: &Locale,
    payload: AccessProfileJson,
    tenant_id: Option<i64>,
    uuid: Option<String>,
) -> Result<AccessProfile, ExceptionResponse> {
    let base_role = Role::from_str(&payload.base_role)
        .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue))?;
    Ok(AccessProfile {
        id: None,
        uuid,
        tenant_id,
        name: payload.name,
        name_key: String::new(),
        base_role,
        active: payload.active,
        permissions: payload.permissions,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    })
}

#[utoipa::path(
    get,
    tag = "AccessProfile",
    path = "/access-profile/catalogue",
    responses(
        (status = 200, description = "The platform's grantable permissions, one per resource and action, grouped by module, each with the roles whose built-in default holds it (HRMS-850). **Roles:** SysAdmin, TenantOwner.", body = Vec<PermissionJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Only the tenant owner manages access profiles", body = ForbiddenErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn catalogue(
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<Vec<PermissionJson>>> {
    if !can_manage_access_profiles(&current_user, current_user.tenant_id.or(Some(0))) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::AccessProfileForbidden));
    }
    Ok(Json(
        GRANTABLE
            .iter()
            .map(|p| PermissionJson {
                code: p.code.to_string(),
                module: p.module.to_string(),
                own: p.own,
                default_for: [("TenantUser", p.tenant_user), ("Driver", p.driver), ("Mechanic", p.mechanic)]
                    .into_iter()
                    .filter(|(_, held)| *held)
                    .map(|(role, _)| role.to_string())
                    .collect(),
            })
            .collect(),
    ))
}

#[utoipa::path(
    get,
    tag = "AccessProfile",
    path = "/access-profile/mine",
    responses(
        (status = 200, description = "What the caller may do now: the owner and the platform administrator everything, anyone else their profile's permissions or the built-in default of their role, plus the baseline. Resolved on every call, so a change takes effect on the next request (HRMS-853). **Roles:** any authenticated role.", body = EffectivePermissionsJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn mine(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<EffectivePermissionsJson>> {
    match use_case(&state).effective(&current_user).await {
        Ok(e) => Ok(Json(EffectivePermissionsJson {
            role: e.role.to_string(),
            profile_uuid: e.profile_uuid,
            profile_name: e.profile_name,
            permissions: e.permissions,
        })),
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}

#[utoipa::path(
    post,
    tag = "AccessProfile",
    path = "/access-profile",
    request_body = AccessProfileJson,
    responses(
        (status = 201, description = "The profile is created for the tenant (HRMS-851). **Roles:** SysAdmin (unbound; names the owning tenant); TenantOwner (own tenant only).", body = AccessProfileJson),
        (status = 400, description = "Blank name, a role that takes no profile, or a permission outside the catalogue", body = BadRequestErrorJson),
        (status = 409, description = "`DuplicateAccessProfile`: the tenant already has a profile with this normalised name", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Only the tenant owner manages access profiles", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<AccessProfileJson>,
) -> HttpResponse<(StatusCode, Json<AccessProfileJson>)> {
    if !can_manage_access_profiles(&current_user, payload.tenant_id.or(current_user.tenant_id)) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::AccessProfileForbidden));
    }
    let tenant_id = current_user.tenant_id.or(payload.tenant_id);
    let profile = domain(&locale, payload, tenant_id, None)?;
    match use_case(&state).create(profile).await {
        Ok(saved) => Ok((StatusCode::CREATED, Json(json(saved)))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    put,
    tag = "AccessProfile",
    path = "/access-profile/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Access profile UUID")),
    request_body = AccessProfileJson,
    responses(
        (status = 200, description = "The profile's name, role, active flag and permissions are replaced; the change reaches its holders on their next request (HRMS-853). **Roles:** SysAdmin, TenantOwner (own tenant only).", body = AccessProfileJson),
        (status = 400, description = "Blank name or a permission outside the catalogue", body = BadRequestErrorJson),
        (status = 409, description = "Duplicate name, or a profile in use cannot be deactivated or moved to another role", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Only the tenant owner manages access profiles", body = ForbiddenErrorJson),
        (status = 404, description = "Profile not found, **or in another tenant** (PD-034)", body = NotFoundErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn update(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<AccessProfileJson>,
) -> HttpResponse<Json<AccessProfileJson>> {
    let current = use_case(&state)
        .find_by_uuid(uuid.clone())
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::AccessProfileNotFound))?;
    if !can_manage_access_profiles(&current_user, current.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::AccessProfileForbidden));
    }
    let edited = domain(&locale, payload, current.tenant_id, Some(uuid))?;
    match use_case(&state).update(edited).await {
        Ok(saved) => Ok(Json(json(saved))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "AccessProfile",
    path = "/access-profile/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Access profile UUID")),
    responses(
        (status = 200, description = "The profile. **Roles:** SysAdmin, TenantOwner (own tenant only).", body = AccessProfileJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Only the tenant owner manages access profiles", body = ForbiddenErrorJson),
        (status = 404, description = "Profile not found, **or in another tenant** (PD-034)", body = NotFoundErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_by_uuid(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<AccessProfileJson>> {
    let profile = use_case(&state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::AccessProfileNotFound))?;
    if !can_manage_access_profiles(&current_user, profile.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::AccessProfileForbidden));
    }
    Ok(Json(json(profile)))
}

#[utoipa::path(
    get,
    tag = "AccessProfile",
    path = "/access-profile",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of the tenant's profiles (PD-028). **Roles:** SysAdmin, TenantOwner (own tenant only).", body = PageJson<AccessProfileJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Only the tenant owner manages access profiles", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_all(
    state: State<AppState>,
    Query(page_query): Query<PageQuery>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<PageJson<AccessProfileJson>>> {
    if !can_manage_access_profiles(&current_user, current_user.tenant_id.or(Some(0))) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::AccessProfileForbidden));
    }
    let (page, page_size) = (page_query.page(), page_query.page_size());
    match use_case(&state).find_page(page, page_size).await {
        Ok((profiles, total)) => Ok(Json(PageJson::new(profiles.into_iter().map(json).collect(), page, page_size, total))),
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}

#[utoipa::path(
    get,
    tag = "AccessProfile",
    path = "/access-profile/user/uuid/{user_uuid}",
    params(("user_uuid" = String, Path, description = "User UUID")),
    responses(
        (status = 200, description = "What the given user may do now -- their profile (if any) and permissions. **Roles:** SysAdmin, TenantOwner (own tenant only).", body = EffectivePermissionsJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Only the tenant owner manages access profiles", body = ForbiddenErrorJson),
        (status = 404, description = "User not found, **or in another tenant** (PD-034)", body = NotFoundErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn user_permissions(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(user_uuid): Path<String>,
) -> HttpResponse<Json<EffectivePermissionsJson>> {
    let user = UserGateway::new(state.conn.as_ref().clone())
        .get_by_uuid(user_uuid)
        .await
        .ok()
        .flatten()
        .ok_or_else(|| ExceptionResponse::NotFound(locale.clone(), ErrorKey::AccessProfileNotFound))?;
    if !can_manage_access_profiles(&current_user, user.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::AccessProfileForbidden));
    }
    match use_case(&state).effective(&user).await {
        Ok(e) => Ok(Json(EffectivePermissionsJson {
            role: e.role.to_string(),
            profile_uuid: e.profile_uuid,
            profile_name: e.profile_name,
            permissions: e.permissions,
        })),
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}

#[utoipa::path(
    put,
    tag = "AccessProfile",
    path = "/access-profile/user/uuid/{user_uuid}",
    params(("user_uuid" = String, Path, description = "User UUID")),
    request_body = AssignProfileJson,
    responses(
        (status = 204, description = "The user holds the profile, or their role's default when none is given (HRMS-852). **Roles:** SysAdmin, TenantOwner (own tenant only)."),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Only the tenant owner manages access profiles", body = ForbiddenErrorJson),
        (status = 404, description = "User or profile not found, **or in another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 409, description = "The profile is for another role or inactive, or the user's role takes no profile", body = BadRequestErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn assign(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(user_uuid): Path<String>,
    Json(payload): Json<AssignProfileJson>,
) -> HttpResponse<StatusCode> {
    let user = UserGateway::new(state.conn.as_ref().clone())
        .get_by_uuid(user_uuid)
        .await
        .ok()
        .flatten()
        .ok_or_else(|| ExceptionResponse::NotFound(locale.clone(), ErrorKey::AccessProfileNotFound))?;
    if !can_manage_access_profiles(&current_user, user.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::AccessProfileForbidden));
    }
    match use_case(&state).assign(&user, payload.profile_uuid).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) if e.message == business::use_cases::access_profile_use_case::PROFILE_NOT_FOUND => {
            Err(ExceptionResponse::NotFound(locale, ErrorKey::AccessProfileNotFound))
        }
        Err(e) => Err(error(locale, &e.message)),
    }
}
