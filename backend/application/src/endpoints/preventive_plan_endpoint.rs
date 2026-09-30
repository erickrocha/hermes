use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::json::preventive_plan_json::{
    PreventiveAlertJson, PreventiveExtensionJson, PreventiveExtensionRequestJson, PreventivePlanJson, PreventiveWorkOrderJson,
};
use crate::endpoints::vehicle_endpoint::find_visible;
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::commons::functions::bytes_para_string;
use business::commons::gateway::Gateway;
use business::domain::authorization::{can_create_preventive_plan, can_read_preventive_plan};
use business::domain::enums::{PreventiveControlType, PreventiveStatus};
use business::domain::preventive_plan::PreventivePlan;
use business::domain::user::User;
use business::gateway::preventive_plan_alert_gateway::PreventivePlanAlertGateway;
use business::gateway::preventive_plan_gateway::PreventivePlanGateway;
use business::use_cases::preventive_plan_alert_use_case::{
    ALERT_INVALID, ALERT_NOT_FOUND, ALERT_NOT_PENDING, DUPLICATE_ALERT, PreventivePlanAlertUseCase,
};
use business::domain::preventive_plan_alert::PreventivePlanAlert;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::gateway::preventive_plan_extension_gateway::PreventivePlanExtensionGateway;
use business::gateway::work_order_gateway::WorkOrderGateway;
use business::gateway::work_order_item_gateway::WorkOrderItemGateway;
use business::use_cases::preventive_plan_use_case::{
    DUPLICATE_PLAN, EXTENSION_DESCRIPTION_REQUIRED, EXTENSION_GRANT_INVALID, EXTENSION_ITEM_NOT_FOUND,
    EXTENSION_ITEM_NOT_LINKED, EXTENSION_ITEM_NOT_PENDING, INTERVAL_REQUIRED, ORIGIN_ORDER_NOT_ON_VEHICLE, PLAN_NAME_REQUIRED, PLAN_NOT_DUE, PREVENTIVE_PLAN_NOT_FOUND,
    PreventivePlanUseCase, VEHICLE_NOT_FOUND,
};
use std::str::FromStr;

fn use_case(state: &AppState) -> PreventivePlanUseCase {
    let db = state.conn.as_ref().clone();
    PreventivePlanUseCase::new(
        PreventivePlanGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        WorkOrderGateway::new(db.clone()),
        WorkOrderItemGateway::new(db.clone()),
        PreventivePlanExtensionGateway::new(db.clone()),
        business::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway::new(db),
    )
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        DUPLICATE_PLAN => ExceptionResponse::BadRequest(locale, ErrorKey::DuplicatePreventivePlan),
        EXTENSION_ITEM_NOT_FOUND => ExceptionResponse::NotFound(locale, ErrorKey::WorkOrderNotFound),
        ORIGIN_ORDER_NOT_ON_VEHICLE | EXTENSION_GRANT_INVALID | EXTENSION_DESCRIPTION_REQUIRED | EXTENSION_ITEM_NOT_LINKED | EXTENSION_ITEM_NOT_PENDING => {
            ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)
        }
        PLAN_NOT_DUE => ExceptionResponse::BadRequest(locale, ErrorKey::PreventivePlanNotDue),
        PREVENTIVE_PLAN_NOT_FOUND => ExceptionResponse::NotFound(locale, ErrorKey::PreventivePlanNotFound),
        VEHICLE_NOT_FOUND | PLAN_NAME_REQUIRED | INTERVAL_REQUIRED => {
            ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)
        }
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

async fn json(state: &AppState, plan: PreventivePlan, status: PreventiveStatus) -> PreventivePlanJson {
    let vehicle_uuid = VehicleGateway::new(state.conn.as_ref().clone())
        .find_by_id(plan.vehicle_id)
        .await
        .ok()
        .flatten()
        .map(|m| bytes_para_string(m.uuid))
        .unwrap_or_default();

    let last_work_order_uuid = match plan.last_work_order_id {
        Some(id) => WorkOrderGateway::new(state.conn.as_ref().clone())
            .find_by_id(id)
            .await
            .ok()
            .flatten()
            .map(|m| bytes_para_string(m.uuid)),
        None => None,
    };

    PreventivePlanJson {
        uuid: plan.uuid,
        tenant_id: plan.tenant_id,
        vehicle_uuid,
        plan_name: plan.plan_name,
        control_type: plan.control_type.to_string(),
        interval_km: plan.interval_km,
        interval_days: plan.interval_days,
        last_service_km: plan.last_service_km,
        last_service_date: plan.last_service_date,
        extension_limit_km: plan.extension_limit_km,
        last_work_order_uuid,
        status: Some(status.to_string()),
    }
}

#[utoipa::path(
    post,
    tag = "PreventivePlan",
    path = "/preventive-plan",
    request_body = PreventivePlanJson,
    responses(
        (status = 201, description = "The preventive plan is registered (TRM-300/301). `status` is server-derived, ignored on write. **Roles:** SysAdmin (unbound; names the owning tenant); TenantOwner, Mechanic (own tenant only).", body = PreventivePlanJson),
        (status = 400, description = "Bad request, including a blank name, a missing interval for the chosen control type, an unknown vehicle, or a duplicate plan name for this vehicle (TRM-300)", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not manage preventive plans", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<PreventivePlanJson>,
) -> HttpResponse<(StatusCode, Json<PreventivePlanJson>)> {
    if !can_create_preventive_plan(&current_user, payload.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::PreventivePlanForbidden));
    }

    let mut tenant_id = payload.tenant_id;
    if current_user.tenant_id.is_some() {
        tenant_id = current_user.tenant_id;
    }

    let vehicle = find_visible(&state, &locale, &current_user, payload.vehicle_uuid).await?;
    let control_type = PreventiveControlType::from_str(&payload.control_type).unwrap_or_default();

    let plan = PreventivePlan {
        id: None,
        uuid: None,
        tenant_id,
        vehicle_id: vehicle.id.unwrap_or_default(),
        plan_name: payload.plan_name,
        control_type,
        interval_km: payload.interval_km,
        interval_days: payload.interval_days,
        last_service_km: payload.last_service_km,
        last_service_date: payload.last_service_date,
        extension_limit_km: None,
        last_work_order_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };

    match use_case(&state).create(plan).await {
        Ok(saved) => {
            // Recomputes status fresh (TRM-302/303) rather than assuming a
            // freshly created plan is always `Ok` -- an imported plan can be
            // created already overdue.
            let (plan, status) = use_case(&state)
                .find_by_uuid(saved.uuid.clone().unwrap_or_default())
                .await
                .unwrap_or((saved, PreventiveStatus::default()));
            Ok((StatusCode::CREATED, Json(json(&state, plan, status).await)))
        }
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "PreventivePlan",
    path = "/preventive-plan/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Preventive plan UUID")),
    responses(
        (status = 200, description = "The preventive plan, with its status freshly computed against the vehicle's current odometer and today's date (TRM-302/303). **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = PreventivePlanJson),
        (status = 404, description = "Preventive plan not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_by_uuid(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<PreventivePlanJson>> {
    let (plan, status) = use_case(&state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::PreventivePlanNotFound))?;
    if !can_read_preventive_plan(&current_user, plan.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::PreventivePlanNotFound));
    }
    Ok(Json(json(&state, plan, status).await))
}

#[utoipa::path(
    get,
    tag = "PreventivePlan",
    path = "/preventive-plan",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of the caller's own tenant's preventive plans (PD-028), by vehicle. **Roles:** SysAdmin, TenantOwner, TenantUser, Driver, Mechanic (any authenticated role; SysAdmin sees every tenant, everyone else only their own).", body = PageJson<PreventivePlanJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_all(
    state: State<AppState>,
    Query(page_query): Query<PageQuery>,
    Extension(locale): Extension<Locale>,
    Extension(_current_user): Extension<User>,
) -> HttpResponse<Json<PageJson<PreventivePlanJson>>> {
    let (page, page_size) = (page_query.page(), page_query.page_size());
    match use_case(&state).find_page(page, page_size).await {
        Ok((plans, total)) => {
            let mut rows = Vec::with_capacity(plans.len());
            for (plan, status) in plans {
                rows.push(json(&state, plan, status).await);
            }
            Ok(Json(PageJson::new(rows, page, page_size, total)))
        }
        Err(_) => Err(ExceptionResponse::InternalServerError(
            locale,
            ErrorKey::UnexpectedError,
        )),
    }
}

#[utoipa::path(
    post,
    tag = "PreventivePlan",
    path = "/preventive-plan/uuid/{uuid}/work-order",
    params(("uuid" = String, Path, description = "Preventive plan UUID")),
    responses(
        (status = 201, description = "A work order (origin `Preventive`) was opened for the due plan (TRM-306). **Roles:** SysAdmin, TenantOwner, Mechanic (own tenant only).", body = PreventiveWorkOrderJson),
        (status = 200, description = "The plan already has an open work order; it is returned, nothing is created (`created` is false)", body = PreventiveWorkOrderJson),
        (status = 400, description = "The plan is not overdue and not within its attention margin", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not manage preventive plans", body = ForbiddenErrorJson),
        (status = 404, description = "Plan not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn generate_work_order(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<(StatusCode, Json<PreventiveWorkOrderJson>)> {
    let (plan, _) = use_case(&state)
        .find_by_uuid(uuid.clone())
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::PreventivePlanNotFound))?;
    if !can_read_preventive_plan(&current_user, plan.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::PreventivePlanNotFound));
    }
    if !can_create_preventive_plan(&current_user, plan.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::PreventivePlanForbidden));
    }

    match use_case(&state).generate_work_order(uuid).await {
        Ok((work_order, created)) => {
            let status = if created { StatusCode::CREATED } else { StatusCode::OK };
            Ok((
                status,
                Json(PreventiveWorkOrderJson {
                    number: work_order.id.map(|id| format!("OS-{id}")),
                    work_order_uuid: work_order.uuid,
                    created,
                }),
            ))
        }
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    post,
    tag = "PreventivePlan",
    path = "/preventive-plan/uuid/{uuid}/extension",
    params(("uuid" = String, Path, description = "Preventive plan UUID")),
    request_body = PreventiveExtensionRequestJson,
    responses(
        (status = 201, description = "The plan is extended after a technical inspection (TRM-312…320): its kilometre limit becomes the inspection odometer plus the granted kilometres, the inspected item is resolved with a narrative, its work order is left partially resolved for conferral, and an auditable entry is kept. Does not renew the cycle (TRM-311). **Roles:** SysAdmin, TenantOwner, Mechanic (own tenant only).", body = PreventiveExtensionJson),
        (status = 400, description = "Grant not positive or above 50,000 km, blank description, an item not linked to this plan / another vehicle's order, or an item that is not pending", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not manage preventive plans", body = ForbiddenErrorJson),
        (status = 404, description = "Plan or item not found, **or in another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn extend(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<PreventiveExtensionRequestJson>,
) -> HttpResponse<(StatusCode, Json<PreventiveExtensionJson>)> {
    let (plan, _) = use_case(&state)
        .find_by_uuid(uuid.clone())
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::PreventivePlanNotFound))?;
    if !can_read_preventive_plan(&current_user, plan.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::PreventivePlanNotFound));
    }
    if !can_create_preventive_plan(&current_user, plan.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::PreventivePlanForbidden));
    }

    match use_case(&state)
        .extend(uuid, payload.work_order_item_uuid, payload.inspection_km, payload.granted_km, payload.description)
        .await
    {
        Ok(entry) => Ok((
            StatusCode::CREATED,
            Json(PreventiveExtensionJson {
                uuid: entry.uuid,
                inspection_km: entry.inspection_km,
                granted_km: entry.granted_km,
                resulting_limit_km: entry.resulting_limit_km,
                description: entry.description,
            }),
        )),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    put,
    tag = "PreventivePlan",
    path = "/preventive-plan/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Preventive plan UUID")),
    request_body = PreventivePlanJson,
    responses(
        (status = 200, description = "The plan's control type, intervals and base (last service km/date) are replaced (TRM-330). The name and vehicle never change. Changing the base starts a new cycle: any active extension ends, its entries are kept. **Roles:** SysAdmin, TenantOwner, Mechanic (own tenant only).", body = PreventivePlanJson),
        (status = 400, description = "The control type lacks its matching interval", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not manage preventive plans", body = ForbiddenErrorJson),
        (status = 404, description = "Plan not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn update(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<PreventivePlanJson>,
) -> HttpResponse<Json<PreventivePlanJson>> {
    let (current, _) = use_case(&state)
        .find_by_uuid(uuid.clone())
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::PreventivePlanNotFound))?;
    if !can_read_preventive_plan(&current_user, current.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::PreventivePlanNotFound));
    }
    if !can_create_preventive_plan(&current_user, current.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::PreventivePlanForbidden));
    }

    let last_work_order_id = match payload.last_work_order_uuid {
        Some(order_uuid) => WorkOrderGateway::new(state.conn.as_ref().clone())
            .find_by_uuid(order_uuid)
            .await
            .map_err(|_| ExceptionResponse::InternalServerError(locale.clone(), ErrorKey::UnexpectedError))?
            .map(|m| m.id)
            .ok_or_else(|| ExceptionResponse::NotFound(locale.clone(), ErrorKey::WorkOrderNotFound))?
            .into(),
        None => None,
    };
    let edited = PreventivePlan {
        uuid: Some(uuid),
        last_work_order_id,
        control_type: PreventiveControlType::from_str(&payload.control_type).unwrap_or_default(),
        interval_km: payload.interval_km,
        interval_days: payload.interval_days,
        last_service_km: payload.last_service_km,
        last_service_date: payload.last_service_date,
        ..current
    };
    match use_case(&state).update(edited).await {
        Ok(saved) => {
            let (plan, status) = use_case(&state)
                .find_by_uuid(saved.uuid.clone().unwrap_or_default())
                .await
                .unwrap_or((saved, PreventiveStatus::default()));
            Ok(Json(json(&state, plan, status).await))
        }
        Err(e) => Err(error(locale, &e.message)),
    }
}

fn alert_use_case(state: &AppState) -> PreventivePlanAlertUseCase {
    let db = state.conn.as_ref().clone();
    PreventivePlanAlertUseCase::new(
        PreventivePlanAlertGateway::new(db.clone()),
        PreventivePlanGateway::new(db.clone()),
        VehicleGateway::new(db),
    )
}

fn alert_json(a: PreventivePlanAlert) -> PreventiveAlertJson {
    PreventiveAlertJson { uuid: a.uuid, at_km: a.at_km, title: a.title, inspection_model: a.inspection_model }
}

fn alert_error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        ALERT_NOT_FOUND => ExceptionResponse::NotFound(locale, ErrorKey::PreventiveAlertNotFound),
        PREVENTIVE_PLAN_NOT_FOUND => ExceptionResponse::NotFound(locale, ErrorKey::PreventivePlanNotFound),
        ALERT_INVALID | DUPLICATE_ALERT | ALERT_NOT_PENDING => {
            ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)
        }
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

#[utoipa::path(
    post,
    tag = "PreventivePlan",
    path = "/preventive-plan/uuid/{uuid}/alert",
    params(("uuid" = String, Path, description = "Preventive plan UUID")),
    request_body = PreventiveAlertJson,
    responses(
        (status = 201, description = "An intermediate inspection alert is declared on the plan, `atKm` kilometres into its cycle (TRM-323). **Roles:** SysAdmin, TenantOwner, Mechanic (own tenant only).", body = PreventiveAlertJson),
        (status = 400, description = "Non-positive distance, blank title or inspection model, or an alert already at this distance", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not manage preventive plans", body = ForbiddenErrorJson),
        (status = 404, description = "Plan not found, **or in another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add_alert(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<PreventiveAlertJson>,
) -> HttpResponse<(StatusCode, Json<PreventiveAlertJson>)> {
    let (plan, _) = use_case(&state)
        .find_by_uuid(uuid.clone())
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::PreventivePlanNotFound))?;
    if !can_read_preventive_plan(&current_user, plan.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::PreventivePlanNotFound));
    }
    if !can_create_preventive_plan(&current_user, plan.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::PreventivePlanForbidden));
    }
    match alert_use_case(&state).add(uuid, payload.at_km, payload.title, payload.inspection_model).await {
        Ok(a) => Ok((StatusCode::CREATED, Json(alert_json(a)))),
        Err(e) => Err(alert_error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "PreventivePlan",
    path = "/preventive-plan/uuid/{uuid}/alert",
    params(("uuid" = String, Path, description = "Preventive plan UUID")),
    responses(
        (status = 200, description = "The plan's pending intermediate alerts (TRM-324): kilometres driven since the last service have reached the threshold and it is not discharged for the current cycle, ascending by kilometre. **Roles:** any caller of the plan's own tenant; SysAdmin.", body = Vec<PreventiveAlertJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 404, description = "Plan not found, **or in another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn pending_alerts(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<Vec<PreventiveAlertJson>>> {
    let (plan, _) = use_case(&state)
        .find_by_uuid(uuid.clone())
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::PreventivePlanNotFound))?;
    if !can_read_preventive_plan(&current_user, plan.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::PreventivePlanNotFound));
    }
    match alert_use_case(&state).pending(uuid).await {
        Ok(alerts) => Ok(Json(alerts.into_iter().map(alert_json).collect())),
        Err(e) => Err(alert_error(locale, &e.message)),
    }
}

#[utoipa::path(
    post,
    tag = "PreventivePlan",
    path = "/preventive-plan/alert/{uuid}/discharge",
    params(("uuid" = String, Path, description = "Alert UUID")),
    responses(
        (status = 200, description = "The alert is discharged for the plan's current cycle; a real service reopens it (TRM-325). **Roles:** SysAdmin, TenantOwner, Mechanic (own tenant only).", body = PreventiveAlertJson),
        (status = 400, description = "The alert is not pending in the current cycle", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not manage preventive plans", body = ForbiddenErrorJson),
        (status = 404, description = "Alert not found, **or in another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn discharge_alert(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<PreventiveAlertJson>> {
    let alert = PreventivePlanAlertGateway::new(state.conn.as_ref().clone())
        .find_by_uuid(uuid.clone())
        .await
        .ok()
        .flatten()
        .ok_or_else(|| ExceptionResponse::NotFound(locale.clone(), ErrorKey::PreventiveAlertNotFound))?;
    if !can_read_preventive_plan(&current_user, alert.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::PreventiveAlertNotFound));
    }
    if !can_create_preventive_plan(&current_user, alert.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::PreventivePlanForbidden));
    }
    match alert_use_case(&state).discharge(uuid).await {
        Ok(a) => Ok(Json(alert_json(a))),
        Err(e) => Err(alert_error(locale, &e.message)),
    }
}
