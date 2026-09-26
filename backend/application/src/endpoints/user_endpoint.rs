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
use crate::infrastructure::mapper::{Mapper, UserMapper, reject_unknown_role};
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::commons::email_sender::EmailSender;
use business::domain::authorization::{
    CreationTenant, can_administer_user, can_create_user_with_role, can_read_user_record,
    can_reassign_role,
};
use business::domain::enums::Role;
use business::domain::user::User;
use business::gateway::tenant_gateway::TenantGateway;
use business::gateway::user_gateway::UserGateway;
use business::use_cases::account_invite_use_case::AccountInviteUseCase;
use business::use_cases::tenant_use_case::TenantUseCase;
use business::use_cases::user_use_case::UserUseCase;
use std::env;

#[utoipa::path(
    post,
    tag = "User",
    path = "/user",
    request_body = UserJson,
    responses(
        (status = 201, description = "User created. **Roles:** SysAdmin (unbound; creates SysAdmin or TenantOwner accounts), TenantOwner (creates TenantUser, Driver or Mechanic accounts in their own tenant).", body = UserJson),
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
    reject_unknown_role(&payload.role, &locale)?;
    let mut domain = UserMapper::domain(payload);

    // PD-019's creation hierarchy (EPIC-IA-04): an unbound platform
    // administrator creates administrators and tenant owners; a tenant
    // owner creates tenant users in their own tenant only. See
    // business::domain::authorization for why this is not the rule the
    // code used to implement.
    let Some(creation_tenant) = can_create_user_with_role(&current_user, &domain.role) else {
        // DEF-BO-05: the refusal used to be `RequiredHeaderValueMissing`, which
        // described a malformed request rather than a role that may not create
        // users — and the console showed that sentence to the person.
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
            // 201 to `{"role":"SysAdmin","tenantId":42}` and stored a
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

    // EPIC-IA-07/D-07 (PD-002, HRM-050/HRMS-123): every account created
    // through this endpoint -- administrator or tenant user alike -- is
    // provisioned, never self-service. It is born with a secret nobody
    // knows and the only way in is the invitation issued below; whatever
    // password the caller sent (if any) is discarded here, never hashed or
    // stored.
    domain.password = AccountInviteUseCase::unguessable_secret();

    let use_case = UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()));
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

    let (refreshed, invite) = AccountInviteUseCase::reissue(state.conn.as_ref(), existing)
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
        (status = 200, description = "A page of users (PD-028). **Roles:** SysAdmin, TenantOwner see their scope's users; TenantUser, Driver, Mechanic receive an empty page (no user-administration rights).", body = PageJson<UserJson>),
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

    // HRM-094/095: quem pode ver o quê não muda com a paginação. Um TenantOwner
    // é filtrado pelo escopo do gateway (`tenant_select`), não por um segundo
    // filtro aqui; um TenantUser continua sem administração de usuários.
    let page_result = match current_user.role {
        Role::SysAdmin | Role::TenantOwner => {
            use_case.find_page(page, page_size, search.as_deref()).await
        }
        _ => Ok((Vec::new(), 0)),
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
        (status = 200, description = "User updated. **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only).", body = UserJson),
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
    reject_unknown_role(&payload.role, &locale)?;
    let mut domain = UserMapper::domain(payload);
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

    // PD-019: reassigning the hierarchy is an unbound platform
    // administrator's decision alone. Everyone else editing an account --
    // including that account's own owner -- keeps its existing role and
    // tenant; only the other fields in the payload take effect.
    if can_reassign_role(&current_user, &domain.role, domain.tenant_id) {
        if domain.role == Role::TenantOwner {
            let tenant_id = domain
                .tenant_id
                .expect("can_reassign_role requires Some for TenantOwner");
            let tenant_use_case =
                TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()));
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
