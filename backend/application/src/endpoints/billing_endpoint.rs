use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::billing_json::{
    BillingChargeJson, BillingSettingsJson, BillingSuspensionJson, ConfigureBillingSettingsJson,
};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::infrastructure::mapper::{Mapper, TenantMapper};
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use business::domain::authorization::{can_access_tenant, can_manage_business_plan_catalogue, can_read_tenant_configuration};
use business::domain::billing::{BillingCharge, TenantBilling};
use business::domain::user::User;
use business::gateway::billing_gateway::BillingGateway;
use business::gateway::business_plan_gateway::BusinessPlanGateway;
use business::gateway::tenant_gateway::TenantGateway;
use business::use_cases::billing_use_case::BillingUseCase;
use business::use_cases::tenant_use_case::TenantUseCase;
use chrono::{Duration, Utc};

fn use_case(state: &AppState) -> BillingUseCase<BillingGateway, TenantGateway, BusinessPlanGateway> {
    BillingUseCase::new(
        BillingGateway::new(state.conn.as_ref().clone()),
        TenantGateway::new(state.conn.as_ref().clone()),
        BusinessPlanGateway::new(state.conn.as_ref().clone()),
    )
}

fn charge_json(charge: BillingCharge, locale: &Locale) -> Result<BillingChargeJson, ExceptionResponse> {
    let uuid = charge.uuid.ok_or_else(|| {
        ExceptionResponse::InternalServerError(locale.clone(), ErrorKey::UnexpectedError)
    })?;
    Ok(BillingChargeJson {
        uuid,
        tenant_uuid: charge.tenant_uuid,
        tenant_name: charge.tenant_name,
        plan_name: charge.plan_name,
        currency: charge.currency,
        period_start: charge.period_start,
        period_end: charge.period_end,
        price_in_cents: charge.price_in_cents,
        price_per_extra_user_in_cents: charge.price_per_extra_user_in_cents,
        included_users: charge.included_users,
        active_users: charge.active_users,
        extra_users: charge.extra_users,
        grace_days: charge.grace_days,
        total_in_cents: charge.total_in_cents,
        status: charge.status.as_str().to_string(),
        is_overdue: charge.status == business::domain::billing::BillingChargeStatus::Pending
            && Utc::now().naive_utc()
                > charge.period_end + Duration::days(i64::from(charge.grace_days)),
        paid_at: charge.paid_at,
        paid_by: charge.paid_by,
    })
}

fn settings_json(tenant_uuid: String, settings: Option<TenantBilling>) -> BillingSettingsJson {
    BillingSettingsJson {
        tenant_uuid,
        grace_days: settings.as_ref().and_then(|settings| settings.grace_days),
        cycle_started_at: settings.as_ref().and_then(|settings| settings.cycle_started_at),
        pending_plan: settings.as_ref().and_then(|settings| settings.pending_business_plan_id).is_some(),
        suspended_at: settings.as_ref().and_then(|settings| settings.suspended_at),
        suspended_by: settings.and_then(|settings| settings.suspended_by),
    }
}

async fn tenant_id_by_uuid(
    state: &AppState,
    locale: &Locale,
    uuid: String,
) -> Result<i64, ExceptionResponse> {
    let tenant = TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()))
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::TenantNotFound))?;
    tenant.id.ok_or_else(|| {
        ExceptionResponse::InternalServerError(locale.clone(), ErrorKey::UnexpectedError)
    })
}

#[utoipa::path(
    get,
    tag = "Billing",
    path = "/billing/charges",
    params(PageQuery),
    responses(
        (status = 200, description = "Billing history. **Roles:** SysAdmin (all tenants); TenantOwner (own tenant).", body = PageJson<BillingChargeJson>),
        (status = 401, body = UnauthorizedErrorJson),
        (status = 403, body = ForbiddenErrorJson),
        (status = 500, body = InternalServerErrorJson)
    ),
    security(("bearer_auth" = []))
)]
pub async fn charges(
    State(state): State<AppState>,
    Query(query): Query<PageQuery>,
    Extension(locale): Extension<Locale>,
    Extension(user): Extension<User>,
) -> HttpResponse<Json<PageJson<BillingChargeJson>>> {
    if !can_read_tenant_configuration(&user) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::BillingForbidden));
    }
    let page = query.page();
    let page_size = query.page_size();
    let (charges, total) = use_case(&state)
        .list_charges(page, page_size)
        .await
        .map_err(|_| ExceptionResponse::InternalServerError(locale.clone(), ErrorKey::UnexpectedError))?;
    let items = charges.into_iter().map(|charge| charge_json(charge, &locale)).collect::<Result<Vec<_>, _>>()?;
    Ok(Json(PageJson::new(items, page, page_size, total)))
}

#[utoipa::path(
    post,
    tag = "Billing",
    path = "/billing/charges/uuid/{uuid}/confirm",
    params(("uuid" = String, Path, description = "Billing charge UUID")),
    responses(
        (status = 200, description = "Payment recorded. **Roles:** SysAdmin (unbound only).", body = BillingChargeJson),
        (status = 404, body = NotFoundErrorJson),
        (status = 401, body = UnauthorizedErrorJson),
        (status = 403, body = ForbiddenErrorJson),
        (status = 500, body = InternalServerErrorJson)
    ),
    security(("bearer_auth" = []))
)]
pub async fn confirm_charge(
    State(state): State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<BillingChargeJson>> {
    if !can_manage_business_plan_catalogue(&user) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::BillingForbidden));
    }
    let charge = use_case(&state)
        .confirm_payment(&uuid, &user.email, Utc::now().naive_utc())
        .await
        .map_err(|error| {
            if error.is_not_found() {
                ExceptionResponse::NotFound(locale.clone(), ErrorKey::BillingChargeNotFound)
            } else {
                ExceptionResponse::InternalServerError(locale.clone(), ErrorKey::UnexpectedError)
            }
        })?;
    Ok(Json(charge_json(charge, &locale)?))
}

#[utoipa::path(
    get,
    tag = "Billing",
    path = "/billing/tenant/uuid/{uuid}/settings",
    params(("uuid" = String, Path, description = "Tenant UUID")),
    responses(
        (status = 200, description = "Billing settings. **Roles:** SysAdmin (any tenant); TenantOwner (own tenant).", body = BillingSettingsJson),
        (status = 404, body = NotFoundErrorJson),
        (status = 401, body = UnauthorizedErrorJson),
        (status = 403, body = ForbiddenErrorJson),
        (status = 500, body = InternalServerErrorJson)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_settings(
    State(state): State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<BillingSettingsJson>> {
    if !can_read_tenant_configuration(&user) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::BillingForbidden));
    }
    let tenant = TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()))
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::TenantNotFound))?;
    let tenant_id = tenant.id.ok_or_else(|| {
        ExceptionResponse::InternalServerError(locale.clone(), ErrorKey::UnexpectedError)
    })?;
    if !can_access_tenant(&user, tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::TenantNotFound));
    }
    let settings = use_case(&state)
        .get_settings(tenant_id)
        .await
        .map_err(|_| ExceptionResponse::InternalServerError(locale.clone(), ErrorKey::UnexpectedError))?;
    Ok(Json(settings_json(TenantMapper::json(tenant).uuid.unwrap_or_default(), settings)))
}

#[utoipa::path(
    put,
    tag = "Billing",
    path = "/billing/tenant/uuid/{uuid}/settings",
    params(("uuid" = String, Path, description = "Tenant UUID")),
    request_body = ConfigureBillingSettingsJson,
    responses(
        (status = 200, description = "Billing settings configured. **Roles:** SysAdmin (unbound only).", body = BillingSettingsJson),
        (status = 400, body = BadRequestErrorJson),
        (status = 404, body = NotFoundErrorJson),
        (status = 401, body = UnauthorizedErrorJson),
        (status = 403, body = ForbiddenErrorJson)
    ),
    security(("bearer_auth" = []))
)]
pub async fn set_grace_days(
    State(state): State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<ConfigureBillingSettingsJson>,
) -> HttpResponse<Json<BillingSettingsJson>> {
    if !can_manage_business_plan_catalogue(&user) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::BillingForbidden));
    }
    let tenant_id = tenant_id_by_uuid(&state, &locale, uuid.clone()).await?;
    let settings = use_case(&state)
        .configure_grace_days(tenant_id, payload.grace_days, Utc::now().naive_utc())
        .await
        .map_err(|error| {
            if error.is_not_found() {
                ExceptionResponse::NotFound(locale.clone(), ErrorKey::TenantNotFound)
            } else {
                ExceptionResponse::BadRequest(locale.clone(), ErrorKey::BillingSettingsInvalid)
            }
        })?;
    Ok(Json(settings_json(uuid, Some(settings))))
}

#[utoipa::path(
    put,
    tag = "Billing",
    path = "/billing/tenant/uuid/{uuid}/suspension",
    params(("uuid" = String, Path, description = "Tenant UUID")),
    request_body = BillingSuspensionJson,
    responses(
        (status = 200, description = "Tenant suspension changed. **Roles:** SysAdmin (unbound only).", body = BillingSettingsJson),
        (status = 404, body = NotFoundErrorJson),
        (status = 401, body = UnauthorizedErrorJson),
        (status = 403, body = ForbiddenErrorJson)
    ),
    security(("bearer_auth" = []))
)]
pub async fn set_suspension(
    State(state): State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<BillingSuspensionJson>,
) -> HttpResponse<Json<BillingSettingsJson>> {
    if !can_manage_business_plan_catalogue(&user) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::BillingForbidden));
    }
    let tenant_id = tenant_id_by_uuid(&state, &locale, uuid.clone()).await?;
    let settings = use_case(&state)
        .set_suspension(tenant_id, payload.suspended, &user.email, Utc::now().naive_utc())
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::TenantNotFound))?;
    Ok(Json(settings_json(uuid, Some(settings))))
}