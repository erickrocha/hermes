use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson, UnauthorizedErrorJson,
};
use crate::endpoints::json::technical_inspection_json::{TechnicalInspectionItemJson, TechnicalInspectionJson};
use crate::endpoints::vehicle_endpoint::find_visible;
use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use business::commons::functions::bytes_para_string;
use business::commons::gateway::Gateway;
use business::domain::authorization::{can_create_preventive_plan, can_read_preventive_plan};
use business::domain::technical_inspection::TechnicalInspection;
use business::domain::technical_inspection_item::{TechnicalInspectionItem};
use business::domain::user::User;
use business::gateway::inspection_model_gateway::InspectionModelGateway;
use business::gateway::preventive_plan_alert_gateway::PreventivePlanAlertGateway;
use business::gateway::preventive_plan_gateway::PreventivePlanGateway;
use business::gateway::technical_inspection_gateway::TechnicalInspectionGateway;
use business::gateway::technical_inspection_item_gateway::TechnicalInspectionItemGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::gateway::work_order_gateway::WorkOrderGateway;
use business::gateway::work_order_item_gateway::WorkOrderItemGateway;
use business::use_cases::technical_inspection_use_case::{INSPECTION_NOT_FOUND, TechnicalInspectionUseCase};

fn use_case(state: &AppState) -> TechnicalInspectionUseCase {
    let db = state.conn.as_ref().clone();
    TechnicalInspectionUseCase::new(
        TechnicalInspectionGateway::new(db.clone()),
        TechnicalInspectionItemGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
        WorkOrderGateway::new(db.clone()),
        WorkOrderItemGateway::new(db.clone()),
        PreventivePlanGateway::new(db.clone()),
        PreventivePlanAlertGateway::new(db.clone()),
        InspectionModelGateway::new(db),
    )
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        INSPECTION_NOT_FOUND => ExceptionResponse::NotFound(locale, ErrorKey::TechnicalInspectionNotFound),
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

async fn json(
    state: &AppState,
    inspection: TechnicalInspection,
    items: Vec<TechnicalInspectionItem>,
    work_order_opened: bool,
) -> Result<TechnicalInspectionJson, String> {
    let vehicle_uuid = VehicleGateway::new(state.conn.as_ref().clone())
        .find_by_id(inspection.vehicle_id)
        .await
        .ok()
        .flatten()
        .map(|m| bytes_para_string(m.uuid))
        .unwrap_or_default();
    let work_order_uuids = use_case(state).work_order_uuids(&items).await.map_err(|e| e.message)?;
    Ok(TechnicalInspectionJson {
        uuid: inspection.uuid,
        vehicle_uuid,
        inspection_model: inspection.inspection_model,
        inspected_at: Some(inspection.inspected_at),
        odometer_km: Some(inspection.odometer_km),
        observation: inspection.observation,
        items: items
            .into_iter()
            .map(|i| TechnicalInspectionItemJson {
                description: i.description,
                conforming: i.conforming,
                observation: i.observation,
            })
            .collect(),
        work_order_uuids,
        work_order_opened,
    })
}

#[utoipa::path(
    post,
    tag = "TechnicalInspection",
    path = "/technical-inspection",
    request_body = TechnicalInspectionJson,
    responses(
        (status = 201, description = "The inspection is recorded (TRM-333). A non-conforming item matching a pending item of an open work order of the same vehicle (normalised description) is noted on that item instead of opening a new order, and renews no preventive plan; one work order is opened for the unmatched ones. `workOrderUuids` names every order touched. Intermediate alerts of the vehicle's plans that name this inspection model are discharged (TRM-325). **Roles:** SysAdmin (unbound; names the owning tenant); TenantOwner, Mechanic (own tenant only).", body = TechnicalInspectionJson),
        (status = 400, description = "Blank inspection model, no items, a blank item description, or an unknown vehicle", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not record inspections", body = ForbiddenErrorJson),
        (status = 404, description = "Vehicle not found, **or in another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<TechnicalInspectionJson>,
) -> HttpResponse<(StatusCode, Json<TechnicalInspectionJson>)> {
    let vehicle = find_visible(&state, &locale, &current_user, payload.vehicle_uuid).await?;
    // The inspection belongs to the vehicle's tenant.
    if !can_create_preventive_plan(&current_user, vehicle.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::PreventivePlanForbidden));
    }
    let inspection = TechnicalInspection {
        id: None,
        uuid: None,
        tenant_id: vehicle.tenant_id,
        vehicle_id: vehicle.id.unwrap_or_default(),
        inspection_model: payload.inspection_model,
        inspected_at: payload.inspected_at.unwrap_or_else(|| chrono::Utc::now().date_naive()),
        odometer_km: payload.odometer_km.unwrap_or(0.0),
        observation: payload.observation,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };
    let answers = payload
        .items
        .into_iter()
        .map(|i| TechnicalInspectionItem {
            id: None,
            uuid: None,
            tenant_id: vehicle.tenant_id,
            technical_inspection_id: 0,
            description: i.description,
            conforming: i.conforming,
            observation: i.observation,
            work_order_item_id: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        })
        .collect();
    match use_case(&state).submit(inspection, answers).await {
        Ok((saved, items, opened)) => match json(&state, saved, items, opened).await {
            Ok(body) => Ok((StatusCode::CREATED, Json(body))),
            Err(message) => Err(error(locale, &message)),
        },
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "TechnicalInspection",
    path = "/technical-inspection/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Technical inspection UUID")),
    responses(
        (status = 200, description = "The inspection, its answers and every work order it touched (TRM-333). **Roles:** any caller of the inspection's own tenant; SysAdmin.", body = TechnicalInspectionJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 404, description = "Inspection not found, **or in another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_by_uuid(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<TechnicalInspectionJson>> {
    let (inspection, items) = use_case(&state).find_by_uuid(uuid).await.map_err(|e| error(locale.clone(), &e.message))?;
    if !can_read_preventive_plan(&current_user, inspection.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::TechnicalInspectionNotFound));
    }
    json(&state, inspection, items, false)
        .await
        .map(Json)
        .map_err(|message| error(locale, &message))
}
