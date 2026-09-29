use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::json::preventive_plan_json::PreventivePlanJson;
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
use business::gateway::preventive_plan_gateway::PreventivePlanGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::use_cases::preventive_plan_use_case::{
    DUPLICATE_PLAN, INTERVAL_REQUIRED, PLAN_NAME_REQUIRED, PreventivePlanUseCase, VEHICLE_NOT_FOUND,
};
use std::str::FromStr;

fn use_case(state: &AppState) -> PreventivePlanUseCase {
    let db = state.conn.as_ref().clone();
    PreventivePlanUseCase::new(PreventivePlanGateway::new(db.clone()), VehicleGateway::new(db))
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        DUPLICATE_PLAN => ExceptionResponse::BadRequest(locale, ErrorKey::DuplicatePreventivePlan),
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
