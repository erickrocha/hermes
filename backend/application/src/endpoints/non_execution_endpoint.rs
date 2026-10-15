use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::non_execution_json::{
    EvaluationJson, JustifyJson, ManualNonExecutionJson, NonExecutionHistoryJson,
    NonExecutionOccurrenceJson, NonExecutionVehicleGroupJson, NonExecutionVehicleJson,
    NonExecutionWorkOrderJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::infrastructure::non_execution_job::{evaluate_all_tenants, use_case};
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::commons::functions::bytes_para_string;
use business::commons::gateway::Gateway;
use business::domain::authorization::{
    can_administer_purchase_order, can_evaluate_non_execution, can_read_non_execution,
};
use business::domain::non_execution::{NonExecutionOccurrence, NonExecutionView};
use business::domain::user::User;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::use_cases::non_execution_use_case::{
    ALREADY_JUSTIFIED, ALREADY_RECORDED, JUSTIFICATION_INVALID, NOT_EVIDENCED, OCCURRENCE_NOT_FOUND,
    WORK_ORDER_NOT_FOUND, WORK_ORDER_NOT_OPEN,
};
use chrono::Utc;

const BY_MAX: usize = 50;

fn ensure(allowed: bool, locale: &Locale) -> Result<(), ExceptionResponse> {
    if allowed {
        Ok(())
    } else {
        Err(ExceptionResponse::Forbidden(locale.clone(), ErrorKey::NonExecutionForbidden))
    }
}

/// Justifying and recording by hand are the maintenance side's acts: platform, owner, mechanic.
fn may_act(user: &User) -> bool {
    can_administer_purchase_order(user, user.tenant_id)
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        WORK_ORDER_NOT_FOUND => ExceptionResponse::NotFound(locale, ErrorKey::WorkOrderNotFound),
        OCCURRENCE_NOT_FOUND => ExceptionResponse::NotFound(locale, ErrorKey::NonExecutionOccurrenceNotFound),
        ALREADY_JUSTIFIED => ExceptionResponse::Conflict(locale, ErrorKey::NonExecutionAlreadyJustified),
        ALREADY_RECORDED => ExceptionResponse::Conflict(locale, ErrorKey::NonExecutionAlreadyRecorded),
        NOT_EVIDENCED => ExceptionResponse::Conflict(locale, ErrorKey::NonExecutionNotEvidenced),
        WORK_ORDER_NOT_OPEN => ExceptionResponse::Conflict(locale, ErrorKey::NonExecutionWorkOrderNotOpen),
        JUSTIFICATION_INVALID => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
        _ => ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError),
    }
}

fn occurrence_json(o: NonExecutionOccurrence) -> NonExecutionOccurrenceJson {
    NonExecutionOccurrenceJson {
        uuid: o.uuid.unwrap_or_default(),
        kind: o.kind.to_string(),
        window_start: o.window_start,
        window_end: o.window_end,
        arrived_at: o.arrived_at,
        departed_at: o.departed_at,
        dwell_minutes: o.dwell_minutes,
        minutes_in_window: o.minutes_in_window,
        justification: o.justification,
        justified_by: o.justified_by,
        justified_at: o.justified_at,
    }
}

fn work_order_json(view: NonExecutionView) -> NonExecutionWorkOrderJson {
    NonExecutionWorkOrderJson {
        work_order_uuid: view.work_order.uuid.unwrap_or_default(),
        description: view.work_order.description,
        status: view.work_order.status.to_string(),
        total_occurrences: view.total_occurrences,
        occurrences: view.occurrences.into_iter().map(occurrence_json).collect(),
    }
}

async fn vehicle_json(state: &AppState, vehicle_id: i64) -> NonExecutionVehicleJson {
    let model = VehicleGateway::new(state.conn.as_ref().clone())
        .find_by_id(vehicle_id)
        .await
        .ok()
        .flatten();
    NonExecutionVehicleJson {
        uuid: model.as_ref().map(|m| bytes_para_string(m.uuid.clone())).unwrap_or_default(),
        plate: model.as_ref().map(|m| m.plate.clone()).unwrap_or_default(),
        model: model.map(|m| m.model).unwrap_or_default(),
    }
}

#[utoipa::path(
    get,
    tag = "NonExecution",
    path = "/non-execution",
    params(PageQuery),
    responses(
        (status = 200, description = "The inbox of planned maintenance that was not carried out (TRM-269), grouped by vehicle -- one row per unresolved work order with only its unjustified occurrences and `totalOccurrences`, the whole history count (TRM-266). A page counts vehicles. **Roles:** SysAdmin (any tenant); TenantOwner, Mechanic, TenantUser (own tenant only).", body = PageJson<NonExecutionVehicleGroupJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not see non-executions", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_all(
    state: State<AppState>,
    Query(page_query): Query<PageQuery>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<PageJson<NonExecutionVehicleGroupJson>>> {
    ensure(can_read_non_execution(&current_user), &locale)?;
    let (page, page_size) = (page_query.page(), page_query.page_size());
    let views = use_case(state.conn.as_ref())
        .pending()
        .await
        .map_err(|e| error(locale, &e.message))?;
    // The views arrive ordered by vehicle, so each vehicle's orders are adjacent.
    let mut groups: Vec<(i64, Vec<NonExecutionView>)> = Vec::new();
    for view in views {
        match groups.last_mut() {
            Some((vehicle_id, rows)) if *vehicle_id == view.record.vehicle_id => rows.push(view),
            _ => groups.push((view.record.vehicle_id, vec![view])),
        }
    }
    let total = groups.len() as u64;
    let mut rows = Vec::new();
    for (vehicle_id, views) in groups
        .into_iter()
        .skip(usize::try_from(page.saturating_mul(page_size)).unwrap_or(usize::MAX))
        .take(usize::try_from(page_size).unwrap_or(usize::MAX))
    {
        rows.push(NonExecutionVehicleGroupJson {
            vehicle: vehicle_json(&state, vehicle_id).await,
            work_orders: views.into_iter().map(work_order_json).collect(),
        });
    }
    Ok(Json(PageJson::new(rows, page, page_size, total)))
}

#[utoipa::path(
    get,
    tag = "NonExecution",
    path = "/non-execution/work-order/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Work order UUID")),
    responses(
        (status = 200, description = "The work order's whole non-execution history, justified occurrences included (TRM-266). **Roles:** SysAdmin (any tenant); TenantOwner, Mechanic, TenantUser (own tenant only).", body = NonExecutionHistoryJson),
        (status = 404, description = "Work order not found, **or it belongs to another tenant**, or it has no history", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not see non-executions", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn history(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<NonExecutionHistoryJson>> {
    ensure(can_read_non_execution(&current_user), &locale)?;
    let view = use_case(state.conn.as_ref())
        .history(uuid)
        .await
        .map_err(|e| error(locale, &e.message))?;
    Ok(Json(NonExecutionHistoryJson {
        vehicle: vehicle_json(&state, view.record.vehicle_id).await,
        work_order: work_order_json(view),
    }))
}

fn actor(user: &User) -> String {
    user.email.chars().take(BY_MAX).collect()
}

#[utoipa::path(
    put,
    tag = "NonExecution",
    path = "/non-execution/occurrence/uuid/{uuid}/justify",
    params(("uuid" = String, Path, description = "Non-execution occurrence UUID")),
    request_body = JustifyJson,
    responses(
        (status = 200, description = "The occurrence, now justified with who and when (TRM-270). The work order's own status never changes (TRM-271). **Roles:** SysAdmin (any tenant); TenantOwner, Mechanic (own tenant only).", body = NonExecutionOccurrenceJson),
        (status = 400, description = "The justification is empty or longer than 1000 characters", body = BadRequestErrorJson),
        (status = 404, description = "Occurrence not found, **or it belongs to another tenant**", body = NotFoundErrorJson),
        (status = 409, description = "The occurrence is already justified", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not justify non-executions", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn justify(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<JustifyJson>,
) -> HttpResponse<Json<NonExecutionOccurrenceJson>> {
    ensure(may_act(&current_user), &locale)?;
    let saved = use_case(state.conn.as_ref())
        .justify(uuid, &payload.justification, &actor(&current_user), Utc::now().naive_utc())
        .await
        .map_err(|e| error(locale, &e.message))?;
    Ok(Json(occurrence_json(saved)))
}

#[utoipa::path(
    post,
    tag = "NonExecution",
    path = "/non-execution/manual",
    request_body = ManualNonExecutionJson,
    responses(
        (status = 201, description = "A `Manual` occurrence, recorded already justified, with the total dwell measured from the arrival and the minutes inside the window (TRM-272/273). **Roles:** SysAdmin (any tenant); TenantOwner, Mechanic (own tenant only).", body = NonExecutionOccurrenceJson),
        (status = 400, description = "The justification is empty or longer than 1000 characters", body = BadRequestErrorJson),
        (status = 404, description = "Work order not found, **or it belongs to another tenant**", body = NotFoundErrorJson),
        (status = 409, description = "The tracker does not evidence it (`NonExecutionNotEvidenced`), the order is already resolved, or it was already recorded", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not record non-executions", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn record_manual(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<ManualNonExecutionJson>,
) -> HttpResponse<(StatusCode, Json<NonExecutionOccurrenceJson>)> {
    ensure(may_act(&current_user), &locale)?;
    let saved = use_case(state.conn.as_ref())
        .record_manual(
            payload.work_order_uuid,
            &payload.justification,
            &actor(&current_user),
            Utc::now().naive_utc(),
        )
        .await
        .map_err(|e| error(locale, &e.message))?;
    Ok((StatusCode::CREATED, Json(occurrence_json(saved))))
}

#[utoipa::path(
    post,
    tag = "NonExecution",
    path = "/non-execution/evaluate",
    responses(
        (status = 200, description = "Runs one idempotent evaluation now, for the caller's tenant (every tenant for the platform administrator), and counts what it did (SR-MT-04-12). **Roles:** SysAdmin; TenantOwner (own tenant only).", body = EvaluationJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Only the tenant owner and the platform administrator evaluate", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn evaluate(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<EvaluationJson>> {
    ensure(can_evaluate_non_execution(&current_user), &locale)?;
    let outcome = if current_user.tenant_id.is_some() {
        use_case(state.conn.as_ref())
            .evaluate(Utc::now().naive_utc())
            .await
            .map_err(|e| error(locale, &e.message))?
    } else {
        evaluate_all_tenants(state.conn.as_ref()).await
    };
    Ok(Json(EvaluationJson {
        plans_evaluated: outcome.plans_evaluated,
        occurrences_recorded: outcome.occurrences_recorded,
        plans_concluded: outcome.plans_concluded,
    }))
}
