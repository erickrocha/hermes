use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::checklist_answer_json::ChecklistAnswerJson;
use crate::endpoints::json::checklist_run_json::ChecklistRunJson;
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::transport_demand_endpoint::resolve_driver_id;
use crate::endpoints::vehicle_endpoint::find_visible;
use crate::infrastructure::mapper::{reject_unknown_answer_status, reject_unknown_checklist_type};
use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use business::commons::entity_mapper::EntityMapper;
use business::commons::gateway::Gateway;
use business::domain::authorization::{can_create_checklist_run, can_read_checklist_run};
use business::domain::checklist_answer::ChecklistAnswer;
use business::domain::checklist_run::{ChecklistRun, ChecklistRunEntityMapper};
use business::domain::checklist_template::ChecklistTemplateEntityMapper;
use business::domain::checklist_template_item::ChecklistTemplateItemEntityMapper;
use business::domain::enums::{AnswerStatus, ChecklistType};
use business::domain::work_order::WorkOrderEntityMapper;
use business::gateway::work_order_gateway::WorkOrderGateway;
use business::gateway::work_order_item_gateway::WorkOrderItemGateway;
use business::domain::user::User;
use business::gateway::checklist_answer_gateway::ChecklistAnswerGateway;
use business::gateway::checklist_run_gateway::ChecklistRunGateway;
use business::gateway::checklist_template_gateway::ChecklistTemplateGateway;
use business::gateway::checklist_template_item_gateway::ChecklistTemplateItemGateway;
use business::gateway::km_evolution_gateway::KmEvolutionGateway;
use business::gateway::user_gateway::UserGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::use_cases::checklist_run_use_case::{
    ChecklistRunUseCase, DRIVER_ALREADY_HOLDS_A_VEHICLE, NOT_A_DRIVER,
};
use business::use_cases::km_evolution_use_case::KmEvolutionUseCase;
use business::use_cases::user_use_case::UserUseCase;
use business::use_cases::vehicle_use_case::VehicleUseCase;
use std::str::FromStr;

fn use_case(state: &AppState) -> ChecklistRunUseCase {
    let db = state.conn.as_ref().clone();
    ChecklistRunUseCase::new(
        ChecklistRunGateway::new(db.clone()),
        ChecklistTemplateGateway::new(db.clone()),
        ChecklistTemplateItemGateway::new(db.clone()),
        ChecklistAnswerGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        UserGateway::new(db.clone()),
        KmEvolutionUseCase::new(KmEvolutionGateway::new(db.clone()), VehicleGateway::new(db.clone())),
        WorkOrderGateway::new(db.clone()),
        WorkOrderItemGateway::new(db),
    )
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        NOT_A_DRIVER => ExceptionResponse::BadRequest(locale, ErrorKey::NotADriver),
        DRIVER_ALREADY_HOLDS_A_VEHICLE => {
            ExceptionResponse::Conflict(locale, ErrorKey::InvalidParameterValue)
        }
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

async fn resolve_checklist_template_id(
    state: &AppState,
    locale: &Locale,
    uuid: String,
) -> Result<i64, ExceptionResponse> {
    let template = ChecklistTemplateGateway::new(state.conn.as_ref().clone())
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue))?;
    template
        .map(|t| t.id)
        .ok_or_else(|| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue))
}

async fn resolve_checklist_template_item_id(
    state: &AppState,
    locale: &Locale,
    uuid: String,
) -> Result<i64, ExceptionResponse> {
    let item = ChecklistTemplateItemGateway::new(state.conn.as_ref().clone())
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue))?;
    item.map(|i| i.id)
        .ok_or_else(|| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue))
}

async fn resolve_opening_checklist_id(
    state: &AppState,
    locale: &Locale,
    uuid: Option<String>,
) -> Result<Option<i64>, ExceptionResponse> {
    let Some(uuid) = uuid else { return Ok(None) };
    let run = ChecklistRunGateway::new(state.conn.as_ref().clone())
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue))?;
    Ok(run.map(|r| r.id))
}

async fn answer_json(state: &AppState, answer: ChecklistAnswer) -> ChecklistAnswerJson {
    let item_uuid = ChecklistTemplateItemGateway::new(state.conn.as_ref().clone())
        .find_by_id(answer.checklist_template_item_id)
        .await
        .ok()
        .flatten()
        .map(ChecklistTemplateItemEntityMapper::from_model)
        .and_then(|i| i.uuid);
    let work_order_uuid = match answer.work_order_id {
        Some(id) => WorkOrderGateway::new(state.conn.as_ref().clone())
            .find_by_id(id)
            .await
            .ok()
            .flatten()
            .map(WorkOrderEntityMapper::from_model)
            .and_then(|w| w.uuid),
        None => None,
    };
    ChecklistAnswerJson {
        uuid: answer.uuid,
        checklist_template_item_uuid: item_uuid.unwrap_or_default(),
        status: answer.status.to_string(),
        observation: answer.observation,
        work_order_uuid,
    }
}

/// `possession`'s reading has no need for a run's answers -- it answers
/// "who holds this vehicle," not "what did the checklist say" -- so it
/// always passes an empty list here rather than paying for the lookup.
async fn json(state: &AppState, run: ChecklistRun, answers: Vec<ChecklistAnswer>) -> ChecklistRunJson {
    let template_uuid = ChecklistTemplateGateway::new(state.conn.as_ref().clone())
        .find_by_id(run.checklist_template_id)
        .await
        .ok()
        .flatten()
        .map(ChecklistTemplateEntityMapper::from_model)
        .and_then(|t| t.uuid);
    let driver_uuid = UserUseCase::new(UserGateway::new(state.conn.as_ref().clone()))
        .find_by_id(run.driver_id)
        .await
        .ok()
        .and_then(|u| u.uuid);
    let vehicle_uuid = VehicleUseCase::new(VehicleGateway::new(state.conn.as_ref().clone()))
        .find_by_id(run.vehicle_id)
        .await
        .ok()
        .and_then(|v| v.uuid);
    let opening_checklist_uuid = match run.opening_checklist_id {
        Some(id) => ChecklistRunGateway::new(state.conn.as_ref().clone())
            .find_by_id(id)
            .await
            .ok()
            .flatten()
            .map(ChecklistRunEntityMapper::from_model)
            .and_then(|r| r.uuid),
        None => None,
    };
    let mut answer_jsons = Vec::with_capacity(answers.len());
    for answer in answers {
        answer_jsons.push(answer_json(state, answer).await);
    }

    ChecklistRunJson {
        uuid: run.uuid,
        checklist_template_uuid: template_uuid.unwrap_or_default(),
        driver_uuid: driver_uuid.unwrap_or_default(),
        vehicle_uuid: vehicle_uuid.unwrap_or_default(),
        checklist_type: run.checklist_type.to_string(),
        odometer_km: run.odometer_km,
        notes: run.notes,
        opening_checklist_uuid,
        answers: answer_jsons,
    }
}

#[utoipa::path(
    post,
    tag = "ChecklistRun",
    path = "/checklist-run",
    request_body = ChecklistRunJson,
    responses(
        (status = 201, description = "The checklist and its answers are recorded together, all-or-nothing, and the vehicle's official odometer record is updated through the one writer EPIC-CK-01-S01 built (HRMS-652/654, AD-041). A Departure opens the vehicle's possession cycle; a Return closes the one it names. Tenant is derived from the named vehicle, never sent by the caller. **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only).", body = ChecklistRunJson),
        (status = 400, description = "Bad request, including a checklist or answer status outside its vocabulary, a relation outside this tenant, a Return with no (or an invalid) opening checklist, no answered items (TRM-101), a duplicate answer for the same item, or an answer naming an item outside this checklist's own template", body = BadRequestErrorJson),
        (status = 409, description = "The driver already holds an open checklist on another vehicle (TRM-105), or the named departure has already been closed", body = BadRequestErrorJson),
        (status = 404, description = "Vehicle not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not submit checklists", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<ChecklistRunJson>,
) -> HttpResponse<(StatusCode, Json<ChecklistRunJson>)> {
    let vehicle = find_visible(&state, &locale, &current_user, payload.vehicle_uuid.clone()).await?;
    if !can_create_checklist_run(&current_user, vehicle.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::ChecklistRunForbidden,
        ));
    }
    reject_unknown_checklist_type(&payload.checklist_type, &locale)?;
    for answer in &payload.answers {
        reject_unknown_answer_status(&answer.status, &locale)?;
    }

    let tenant_id = vehicle.tenant_id;
    let vehicle_id = vehicle.id.unwrap_or_default();
    let checklist_template_id =
        resolve_checklist_template_id(&state, &locale, payload.checklist_template_uuid.clone()).await?;
    let driver_id = resolve_driver_id(&state, &locale, Some(payload.driver_uuid.clone()))
        .await?
        .ok_or_else(|| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::NotADriver))?;
    let opening_checklist_id =
        resolve_opening_checklist_id(&state, &locale, payload.opening_checklist_uuid.clone()).await?;

    let run = ChecklistRun {
        id: None,
        uuid: None,
        tenant_id,
        checklist_template_id,
        driver_id,
        vehicle_id,
        checklist_type: ChecklistType::from_str(&payload.checklist_type)
            .unwrap_or(ChecklistType::Standalone),
        odometer_km: payload.odometer_km,
        notes: payload.notes,
        opening_checklist_id,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };

    let mut answers = Vec::with_capacity(payload.answers.len());
    for answer in payload.answers {
        let checklist_template_item_id =
            resolve_checklist_template_item_id(&state, &locale, answer.checklist_template_item_uuid)
                .await?;
        answers.push(ChecklistAnswer {
            id: None,
            uuid: None,
            tenant_id,
            checklist_run_id: 0,
            checklist_template_item_id,
            status: AnswerStatus::from_str(&answer.status).unwrap_or(AnswerStatus::NonConforming),
            observation: answer.observation,
            work_order_id: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        });
    }

    match use_case(&state).create(run, answers).await {
        Ok((saved_run, saved_answers)) => Ok((
            StatusCode::CREATED,
            Json(json(&state, saved_run, saved_answers).await),
        )),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "ChecklistRun",
    path = "/vehicle/uuid/{uuid}/possession",
    params(("uuid" = String, Path, description = "Vehicle UUID")),
    responses(
        (status = 200, description = "The vehicle's current holder (TRM-123), or null when nobody holds it. `answers` is always empty here; read the checklist by uuid for its answers. **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = Option<ChecklistRunJson>),
        (status = 404, description = "Vehicle not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn possession(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<Option<ChecklistRunJson>>> {
    let vehicle = find_visible(&state, &locale, &current_user, uuid).await?;
    if !can_read_checklist_run(&current_user, vehicle.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::ChecklistRunForbidden,
        ));
    }
    match use_case(&state).current_holder(vehicle.id.unwrap_or_default()).await {
        Ok(Some(run)) => Ok(Json(Some(json(&state, run, Vec::new()).await))),
        Ok(None) => Ok(Json(None)),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "ChecklistRun",
    path = "/checklist-run/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Checklist run UUID")),
    responses(
        (status = 200, description = "The checklist with its answers. **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = ChecklistRunJson),
        (status = 404, description = "Checklist not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
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
) -> HttpResponse<Json<ChecklistRunJson>> {
    let (run, answers) = use_case(&state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::ChecklistRunNotFound))?;
    if !can_read_checklist_run(&current_user, run.tenant_id) {
        return Err(ExceptionResponse::NotFound(
            locale,
            ErrorKey::ChecklistRunNotFound,
        ));
    }
    Ok(Json(json(&state, run, answers).await))
}
