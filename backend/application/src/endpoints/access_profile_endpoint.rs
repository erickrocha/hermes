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
use business::domain::authorization::{can_administer_user, can_manage_access_profiles};
use business::domain::enums::ProfileScope;
use business::domain::permission;
use business::domain::user::User;
use business::gateway::access_profile_gateway::AccessProfileGateway;
use business::gateway::user_gateway::{UserGateway, UserLookupStore};
use business::use_cases::access_profile_use_case::{
    AccessProfileUseCase, DUPLICATE_NAME, EffectivePermissions, GRANT_EXCEEDS_ACTOR, PROFILE_DOES_NOT_FIT,
    PROFILE_IN_USE, PROFILE_NOT_FOUND, SCOPE_NOT_ALLOWED, SYSTEM_PROFILE_READ_ONLY,
};
use std::str::FromStr;

fn use_case(state: &AppState) -> AccessProfileUseCase<AccessProfileGateway> {
    AccessProfileUseCase::new(AccessProfileGateway::new(state.conn.as_ref().clone()))
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        DUPLICATE_NAME => ExceptionResponse::Conflict(locale, ErrorKey::DuplicateAccessProfile),
        PROFILE_IN_USE | PROFILE_DOES_NOT_FIT | SYSTEM_PROFILE_READ_ONLY => {
            ExceptionResponse::Conflict(locale, ErrorKey::InvalidParameterValue)
        }
        PROFILE_NOT_FOUND => ExceptionResponse::NotFound(locale, ErrorKey::AccessProfileNotFound),
        GRANT_EXCEEDS_ACTOR | SCOPE_NOT_ALLOWED => ExceptionResponse::Forbidden(locale, ErrorKey::AccessProfileForbidden),
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

fn json(p: AccessProfile) -> AccessProfileJson {
    AccessProfileJson {
        uuid: p.uuid,
        tenant_id: p.tenant_id,
        name: p.name,
        scope: p.scope.to_string(),
        system: p.system,
        key: p.key,
        active: p.active,
        permissions: p.permissions,
    }
}

fn effective_json(e: EffectivePermissions) -> EffectivePermissionsJson {
    EffectivePermissionsJson {
        scope: e.scope.to_string(),
        profile_uuid: e.profile_uuid,
        profile_name: e.profile_name,
        profile_key: e.profile_key,
        extra_permissions: e.extra_permissions,
        permissions: e.permissions,
    }
}

fn domain(
    locale: &Locale,
    payload: AccessProfileJson,
    tenant_id: Option<i64>,
    uuid: Option<String>,
) -> Result<AccessProfile, ExceptionResponse> {
    let scope = ProfileScope::from_str(&payload.scope)
        .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue))?;
    Ok(AccessProfile {
        id: None,
        uuid,
        tenant_id,
        name: payload.name,
        name_key: String::new(),
        scope,
        system: false,
        key: None,
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
        (status = 200, description = "The permissions the caller may grant, one per resource and action, grouped by module, each with the system profiles that hold it (HRMS-850). A tenant caller sees the tenant permissions; the platform also the platform's. **Roles:** `profile.read`.", body = Vec<PermissionJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller does not administer access profiles", body = ForbiddenErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn catalogue(
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<Vec<PermissionJson>>> {
    if !can_manage_access_profiles(&current_user, current_user.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::AccessProfileForbidden));
    }
    Ok(Json(
        permission::grantable_in(current_user.access.scope)
            .map(|p| PermissionJson {
                code: p.code.to_string(),
                module: p.module.to_string(),
                scope: p.scope.as_str().to_string(),
                own: p.own,
                default_for: permission::default_for(p.code).into_iter().map(String::from).collect(),
            })
            .collect(),
    ))
}

#[utoipa::path(
    get,
    tag = "AccessProfile",
    path = "/access-profile/mine",
    responses(
        (status = 200, description = "What the caller may do now: their profile's grants, their own extras and the baseline. Resolved with the account on every call, so a change takes effect on the next request (HRMS-853). **Roles:** any authenticated account.", body = EffectivePermissionsJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn mine(
    state: State<AppState>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<EffectivePermissionsJson>> {
    Ok(Json(effective_json(use_case(&state).effective(&current_user))))
}

#[utoipa::path(
    post,
    tag = "AccessProfile",
    path = "/access-profile",
    request_body = AccessProfileJson,
    responses(
        (status = 201, description = "The profile is created (HRMS-851). **Roles:** `profile.create` -- a tenant account creates tenant profiles in its own tenant; the platform names the tenant, or creates a platform profile with none. Nobody grants a permission they do not hold (C-035).", body = AccessProfileJson),
        (status = 400, description = "Blank name, an unknown scope, or a grant outside the catalogue or the profile's scope", body = BadRequestErrorJson),
        (status = 409, description = "`DuplicateAccessProfile`: the tenant already has a profile with this normalised name", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller does not administer access profiles here, or grants more than they hold", body = ForbiddenErrorJson),
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
    let tenant_id = current_user.tenant_id.or(payload.tenant_id);
    if !can_manage_access_profiles(&current_user, tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::AccessProfileForbidden));
    }
    let profile = domain(&locale, payload, tenant_id, None)?;
    match use_case(&state).create(&current_user, profile).await {
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
        (status = 200, description = "The profile's name, active flag and grants are replaced; the change reaches its holders on their next request (HRMS-853). **Roles:** `profile.update` in the profile's tenant (the platform anywhere); never a system profile.", body = AccessProfileJson),
        (status = 400, description = "Blank name or a grant outside the catalogue or the profile's scope", body = BadRequestErrorJson),
        (status = 409, description = "Duplicate name, a system profile, or a profile in use cannot be deactivated", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller does not administer access profiles here, or grants more than they hold", body = ForbiddenErrorJson),
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
    match use_case(&state).update(&current_user, edited).await {
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
        (status = 200, description = "The profile. **Roles:** `profile.read` -- the system profiles and the caller's tenant's.", body = AccessProfileJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller does not administer access profiles", body = ForbiddenErrorJson),
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
        (status = 200, description = "A page of the profiles the caller may hand out: the system profiles first, then the tenant's own (PD-028). **Roles:** `profile.read`.", body = PageJson<AccessProfileJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller does not administer access profiles", body = ForbiddenErrorJson),
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
    if !can_manage_access_profiles(&current_user, current_user.tenant_id) {
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
        (status = 200, description = "What the given user may do now -- their profile, extras and effective grants. **Roles:** `profile.read` inside the user's tenant (the platform anywhere).", body = EffectivePermissionsJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller does not administer access profiles", body = ForbiddenErrorJson),
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
    Ok(Json(effective_json(use_case(&state).effective(&user))))
}

#[utoipa::path(
    put,
    tag = "AccessProfile",
    path = "/access-profile/user/uuid/{user_uuid}",
    params(("user_uuid" = String, Path, description = "User UUID")),
    request_body = AssignProfileJson,
    responses(
        (status = 204, description = "The user holds the profile and the extra grants (HRMS-852). **Roles:** `profile.assign` inside the user's tenant (the platform anywhere); the profile must fit the user's tenant and the caller must hold every grant given (C-035)."),
        (status = 400, description = "A grant outside the catalogue or the profile's scope", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller does not administer this user, or grants more than they hold", body = ForbiddenErrorJson),
        (status = 404, description = "User or profile not found, **or in another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 409, description = "The profile does not fit the user's tenant, or is inactive", body = BadRequestErrorJson),
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
    if !can_administer_user(&current_user, user.id, user.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::AccessProfileForbidden));
    }
    match use_case(&state)
        .assign(&current_user, &user, payload.profile_uuid, &payload.extra_permissions)
        .await
    {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err(error(locale, &e.message)),
    }
}
