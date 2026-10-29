use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::change_password_request::ChangePasswordRequest;
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::json::user_json::UserJson;
use crate::infrastructure::mapper::{Mapper, UserMapper};
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::commons::email_sender::EmailSender;
use business::domain::access_profile::AccessProfile;
use business::domain::authorization::{
    CreationTenant, can_administer_user, is_system_administrator, can_assign_profile_in, can_list_users, can_move_user_to_tenant,
    can_read_user_record,
};
use business::commons::tax_id::{TaxIdOutcome, validate_and_normalize};
use business::domain::user::{Access, User};
use business::gateway::access_profile_gateway::AccessProfileGateway;
use business::gateway::tenant_gateway::TenantGateway;
use business::gateway::user_gateway::UserGateway;
use business::use_cases::access_profile_use_case::{
    AccessProfileUseCase, GRANT_EXCEEDS_ACTOR, PROFILE_DOES_NOT_FIT, PROFILE_NOT_FOUND, UNKNOWN_PERMISSION,
};
use business::use_cases::account_invite_use_case::AccountInviteUseCase;
use business::use_cases::tenant_use_case::TenantUseCase;
use business::use_cases::user_use_case::UserUseCase;
use std::env;

/// `C-035`: resolves the profile the payload names (with its extras) into something `actor` may hand to an
/// account in `target_tenant_id`. A profile out of reach, unfit for the tenant, or wider than the actor's
/// own grants is refused: 403 for the hierarchy, 400 for an unknown grant, 404 for no such profile.
async fn placeable_profile(
    state: &AppState,
    locale: &Locale,
    actor: &User,
    profile_uuid: String,
    target_tenant_id: Option<i64>,
    extras: &[String],
) -> Result<(AccessProfile, Vec<String>), ExceptionResponse> {
    AccessProfileUseCase::new(AccessProfileGateway::new(state.conn.as_ref().clone()))
        .placeable(actor, profile_uuid, target_tenant_id, extras)
        .await
        .map_err(|e| match e.message.as_str() {
            PROFILE_NOT_FOUND => ExceptionResponse::NotFound(locale.clone(), ErrorKey::AccessProfileNotFound),
            UNKNOWN_PERMISSION => ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue),
            PROFILE_DOES_NOT_FIT | GRANT_EXCEEDS_ACTOR => {
                ExceptionResponse::Forbidden(locale.clone(), ErrorKey::UserCreationForbidden)
            }
            _ => ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue),
        })
}

#[utoipa::path(
    post,
    tag = "User",
    path = "/user",
    request_body = UserJson,
    responses(
        (status = 201, description = "User created, holding the named profile and extra grants. **Roles:** `user.create` -- the platform hands out platform profiles (no tenant) and tenant profiles (naming the tenant); a tenant account hands out tenant profiles in its own tenant, never granting more than it holds (C-035).", body = UserJson),
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
    let mut domain = UserMapper::domain(payload.clone());
    let Some(profile_uuid) = payload.profile_uuid.filter(|u| !u.trim().is_empty()) else {
        return Err(ExceptionResponse::BadRequest(locale, ErrorKey::RequiredParameterMissing));
    };

    // PD-019's creation hierarchy, restated for profiles (`C-035`): the platform hands out platform
    // profiles and tenant profiles; a tenant account hands out tenant profiles in its own tenant. See
    // business::domain::authorization.
    let profile = AccessProfileUseCase::new(AccessProfileGateway::new(state.conn.as_ref().clone()))
        .find_by_uuid(profile_uuid.clone())
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::AccessProfileNotFound))?;
    let ancestors = AccessProfileUseCase::new(AccessProfileGateway::new(state.conn.as_ref().clone()))
        .ancestors_of(current_user.tenant_id)
        .await
        .map_err(|_| ExceptionResponse::InternalServerError(locale.clone(), ErrorKey::UnexpectedError))?;
    let Some(creation_tenant) = can_assign_profile_in(&current_user, &profile, &ancestors) else {
        // DEF-BO-05: the refusal used to be `RequiredHeaderValueMissing`, which
        // described a malformed request rather than a profile that may not be
        // handed out — and the console showed that sentence to the person.
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::UserCreationForbidden,
        ));
    };

    match creation_tenant {
        CreationTenant::Fixed(tenant_id) => {
            // DEF-IA-08 (HRMS-116, D-08): the hierarchy decides this account's
            // tenant, so a caller who named a different one asked for something
            // the platform will not create. Overwriting it silently answered
            // 201 to `{"profileUuid":<SysAdmin>,"tenantId":42}` and stored a
            // *platform-wide* administrator -- the opposite of what was asked
            // for, with no warning.
            if domain.tenant_id.is_some() && domain.tenant_id != tenant_id {
                return Err(ExceptionResponse::BadRequest(
                    locale,
                    ErrorKey::InvalidParameterValue,
                ));
            }
            domain.tenant_id = tenant_id;
        }
        CreationTenant::CallerChosen => {
            let Some(tenant_id) = domain.tenant_id else {
                return Err(ExceptionResponse::BadRequest(
                    locale,
                    ErrorKey::InvalidParameterValue,
                ));
            };
            let tenant_use_case =
                TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()));
            if tenant_use_case.find_by_id(tenant_id).await.is_err() {
                return Err(ExceptionResponse::BadRequest(
                    locale,
                    ErrorKey::InvalidParameterValue,
                ));
            }
            domain.tenant_id = Some(tenant_id);
        }
    }

    let (profile, extras) =
        placeable_profile(&state, &locale, &current_user, profile_uuid, domain.tenant_id, &domain.extra_permissions).await?;
    domain.profile_id = profile.id;
    domain.extra_permissions = extras;
    domain.access = Access::resolve(&profile, &domain.extra_permissions);

    // EPIC-IA-07/D-07 (PD-002, HRM-050/HRMS-123): every account created
    // through this endpoint -- administrator or tenant user alike -- is
    // provisioned, never self-service. It is born with a secret nobody
    // knows and the only way in is the invitation issued below; whatever
    // password the caller sent (if any) is discarded here, never hashed or
    // stored.
    domain.password = AccountInviteUseCase::unguessable_secret();

    let use_case = UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()));
    check_person_cpf(&state, &use_case, &locale, None, &mut domain).await?;
    let created = match use_case.create(domain).await {
        Ok(user) => user,
        Err(_) => {
            return Err(ExceptionResponse::BadRequest(
                locale,
                ErrorKey::RequiredParameterMissing,
            ));
        }
    };

    issue_and_send_invite(&created);

    Ok((StatusCode::CREATED, Json(UserMapper::json(created))))
}

#[utoipa::path(
    post,
    tag = "User",
    path = "/user/uuid/{uuid}/invite",
    params(
        ("uuid" = String, Path, description = "User UUID")
    ),
    responses(
        (status = 204, description = "A new invitation was issued and any outstanding one was superseded. **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only)."),
        (status = 400, description = "Bad request", body = BadRequestErrorJson),
        (status = 404, description = "User not found, **or it exists outside the caller's boundary**", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
    ),
    security(("bearer_auth" = []))
)]
/// DEF-IA-07 (HRMS-125, D-07, PD-002): re-issues an account's invitation.
///
/// `POST /user` was the only operation that ever issued one, and it cannot run
/// twice for the same address, so an invitation lost to a mail failure or left
/// to expire had no replacement at all -- the only way in was an administrator
/// typing a password for the person, which PD-002 exists to remove.
pub async fn reissue_invite(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<StatusCode> {
    let use_case = UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()));
    let existing = use_case.find_by_uuid(uuid).await.map_err(|_| {
        ExceptionResponse::NotFound(locale.clone(), ErrorKey::RequiredParameterMissing)
    })?;

    // Same boundary as editing the account, and the same 404-not-403 shape:
    // inviting someone is administration of their record.
    if !can_administer_user(&current_user, existing.id, existing.tenant_id) {
        return Err(ExceptionResponse::NotFound(
            locale,
            ErrorKey::RequiredParameterMissing,
        ));
    }

    let (refreshed, invite) =
        AccountInviteUseCase::new(UserGateway::new(state.conn.as_ref().clone()))
            .reissue(existing)
            .await
            .map_err(|_| ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue))?;

    send_invite(&refreshed, invite);

    Ok(StatusCode::NO_CONTENT)
}

/// Best-effort: a mail-server outage must not fail account creation (the
/// account and its 7-day invite token already exist and are valid either
/// way) -- but it must never fail *silently*. Every failure path logs the
/// recipient and reason. See `business::commons::email_sender` for why this
/// isn't fail-fast at boot the way `ACCESS_TOKEN_SECRET` is.
fn issue_and_send_invite(user: &User) {
    let invite = match AccountInviteUseCase::issue(user) {
        Ok(invite) => invite,
        Err(e) => {
            log::error!(
                "[user_endpoint::add] Failed to issue invite for {}: {}",
                user.email,
                e
            );
            return;
        }
    };

    send_invite(user, invite);
}

fn send_invite(user: &User, invite: business::use_cases::account_invite_use_case::AccountInvite) {
    let base_url =
        env::var("BACKOFFICE_BASE_URL").unwrap_or_else(|_| "http://localhost:5173".to_string());
    let accept_url = format!("{base_url}/accept-invite?token={}", invite.token);
    let email = user.email.clone();

    // Detached: the caller (an admin creating a user) gets their 201 back
    // without waiting on a mail server's round trip.
    tokio::spawn(async move {
        match EmailSender::from_env() {
            Ok(sender) => {
                if let Err(e) = sender.send_invite(&email, &accept_url).await {
                    log::error!(
                        "[user_endpoint::send_invite] Failed to send invite email to {}: {}",
                        email,
                        e
                    );
                }
            }
            Err(e) => {
                log::error!(
                    "[user_endpoint::send_invite] Cannot send invite email to {}: {}",
                    email,
                    e
                );
            }
        }
    });
}

#[utoipa::path(
    get,
    tag = "User",
    path = "/user",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of users (PD-028). **Roles:** whoever administers users (`user.create`, `user.update`, `user.invite` or `profile.assign`) sees their scope's users; anyone else receives an empty page.", body = PageJson<UserJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_all(
    state: State<AppState>,
    Query(page_query): Query<PageQuery>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<PageJson<UserJson>>> {
    let use_case = UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()));
    let (page, page_size) = (page_query.page(), page_query.page_size());
    let search = page_query.search();

    // HRM-094/095: quem pode ver o quê não muda com a paginação. Um administrador de usuários
    // é filtrado pelo escopo do gateway (`tenant_select`), não por um segundo filtro aqui; quem
    // não administra usuários continua sem a lista.
    let page_result = if can_list_users(&current_user) {
        use_case.find_page(page, page_size, search.as_deref()).await
    } else {
        Ok((Vec::new(), 0))
    };

    match page_result {
        Ok((users, total)) => Ok(Json(PageJson::new(
            UserMapper::json_vec(users),
            page,
            page_size,
            total,
        ))),
        Err(_) => Ok(Json(PageJson::new(Vec::new(), page, page_size, 0))),
    }
}

#[utoipa::path(
    get,
    tag = "User",
    path = "/user/uuid/{uuid}",
    params(
        ("uuid" = String, Path, description = "User UUID")
    ),
    responses(
        (status = 200, description = "User found. **Roles:** any authenticated role may read their own record; SysAdmin (any tenant) and TenantOwner (own tenant) may read any record in scope.", body = UserJson),
        (status = 404, description = "User not found, **or it exists outside the caller's boundary**", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_by_uuid(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<UserJson>> {
    let use_case = UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()));
    let user = use_case.find_by_uuid(uuid).await.map_err(|_| {
        ExceptionResponse::NotFound(locale.clone(), ErrorKey::RequiredParameterMissing)
    })?;

    // EPIC-IA-05-S01/HRMS-117: a tenant user has no user-administration
    // rights at all -- this used to check only whether a TenantOwner's
    // tenant matched, leaving a TenantUser (or a TenantOwner probing a
    // foreign tenant) free to read any account by id. Reading your *own*
    // record is self-service rather than administration, so this is the
    // read-side rule, which is one case wider than the write-side one.
    if !can_read_user_record(&current_user, user.id, user.tenant_id) {
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
    path = "/user/uuid/{uuid}",
    params(
        ("uuid" = String, Path, description = "User UUID")
    ),
    request_body = UserJson,
    responses(
        (status = 200, description = "User updated. **Roles:** `user.update` inside the account's tenant (the platform anywhere). A new `profileUuid` or `extraPermissions` must be within the caller's reach and grants; only the platform moves an account to another tenant (C-035).", body = UserJson),
        (status = 400, description = "Bad request", body = BadRequestErrorJson),
        (status = 404, description = "User not found, **or it exists outside the caller's boundary**", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn update(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<UserJson>,
) -> HttpResponse<Json<UserJson>> {
    let mut domain = UserMapper::domain(payload.clone());
    let use_case = UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()));

    let existing = use_case.find_by_uuid(uuid).await.map_err(|_| {
        ExceptionResponse::NotFound(locale.clone(), ErrorKey::RequiredParameterMissing)
    })?;
    let id = existing.id.unwrap_or_default();

    if current_user.id == Some(id) && !domain.enabled {
        return Err(ExceptionResponse::BadRequest(
            locale,
            ErrorKey::InvalidParameterValue,
        ));
    }

    // EPIC-IA-05-S01/HRMS-117/HRMS-118: an unbound platform administrator, or
    // the tenant owner of that exact tenant -- nobody else may touch this
    // record at all.
    //
    // DEF-IA-03: "or the target themselves" used to be part of this rule, which
    // turned `PUT /user/{own id}` into a user-administration path open to every
    // role. A tenant user now stops here on their own record too; what they may
    // do to it without administering it lives on `/user/change-password`.
    //
    // Not found rather than forbidden (EPIC-IA-05-S04/HRMS-122's shape,
    // stated for tenant records and extended here to user records for the
    // same reason: a caller outside the boundary should not learn that the
    // id exists at all).
    if !can_administer_user(&current_user, existing.id, existing.tenant_id) {
        return Err(ExceptionResponse::NotFound(
            locale.clone(),
            ErrorKey::RequiredParameterMissing,
        ));
    }

    // `C-035`: the profile and the extras change only through a placement the caller may make; an
    // omitted `profileUuid` keeps the current profile. Moving the account to another tenant (PD-019) is
    // the platform's decision alone; everyone else keeps the account where it is.
    let profile_uuid = payload
        .profile_uuid
        .filter(|u| !u.trim().is_empty())
        .or(existing.access.profile_uuid.clone())
        .ok_or_else(|| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::RequiredParameterMissing))?;
    let profiles = AccessProfileUseCase::new(AccessProfileGateway::new(state.conn.as_ref().clone()));
    let requested = profiles
        .find_by_uuid(profile_uuid.clone())
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::AccessProfileNotFound))?;
    // An omitted `tenantId` keeps the account where it is; a platform profile places it in the platform
    // actor's own tenant, the root (`C-036`).
    let moving_to = match (domain.tenant_id, is_system_administrator(&requested)) {
        (Some(tenant_id), _) if Some(tenant_id) != existing.tenant_id => Some(Some(tenant_id)),
        (None, true) if existing.tenant_id != current_user.tenant_id => Some(current_user.tenant_id),
        _ => None,
    };
    let tenant_id = match moving_to {
        Some(target) => {
            if !can_move_user_to_tenant(&current_user, &requested, target) {
                return Err(ExceptionResponse::Forbidden(locale, ErrorKey::UserCreationForbidden));
            }
            if let Some(tenant_id) = target
                && TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone())).find_by_id(tenant_id).await.is_err()
            {
                return Err(ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue));
            }
            target
        }
        None => existing.tenant_id,
    };
    domain.tenant_id = tenant_id;
    let access_changed = Some(&profile_uuid) != existing.access.profile_uuid.as_ref()
        || domain.extra_permissions != existing.extra_permissions
        || tenant_id != existing.tenant_id;
    if access_changed {
        let (profile, extras) =
            placeable_profile(&state, &locale, &current_user, profile_uuid, tenant_id, &domain.extra_permissions).await?;
        domain.profile_id = profile.id;
        domain.extra_permissions = extras;
        domain.access = Access::resolve(&profile, &domain.extra_permissions);
    } else {
        domain.profile_id = existing.profile_id;
        domain.extra_permissions = existing.extra_permissions.clone();
        domain.access = existing.access.clone();
    }

    check_person_cpf(&state, &use_case, &locale, Some(id), &mut domain).await?;
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
        (status = 200, description = "Password changed. **Roles:** any authenticated role, on their own account only."),
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

    let user_id = current_user
        .id
        .ok_or_else(|| ExceptionResponse::Unauthorized(locale.clone(), ErrorKey::BadCredentials))?;

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

/// `HRMS-861` (`D-27` 6): the CPF is validated by the tenant country's rule where one exists (`PD-022`),
/// kept digits-only, and unique within the tenant. `self_id` is the account being edited, if any.
async fn check_person_cpf(
    state: &AppState,
    use_case: &UserUseCase,
    locale: &Locale,
    self_id: Option<i64>,
    domain: &mut User,
) -> Result<(), ExceptionResponse> {
    let Some(raw) = domain.cpf.take() else {
        return Ok(());
    };
    let bad = || ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue);
    let tenant_id = domain.tenant_id.ok_or_else(bad)?;
    let tenant = TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()))
        .find_by_id(tenant_id)
        .await
        .map_err(|_| bad())?;
    let cpf = match validate_and_normalize(tenant.country_code.as_deref().unwrap_or("BR"), &raw) {
        TaxIdOutcome::Valid(digits) if digits.len() == 11 => digits,
        TaxIdOutcome::Valid(_) | TaxIdOutcome::Invalid => return Err(bad()),
        TaxIdOutcome::Unsupported => raw,
    };
    let taken = use_case
        .find_all_by_tenant_id(tenant_id)
        .await
        .map_err(|_| bad())?
        .iter()
        .any(|u| u.cpf.as_deref() == Some(cpf.as_str()) && u.id != self_id);
    if taken {
        return Err(ExceptionResponse::Conflict(locale.clone(), ErrorKey::DuplicatePersonDocument));
    }
    domain.cpf = Some(cpf);
    Ok(())
}
