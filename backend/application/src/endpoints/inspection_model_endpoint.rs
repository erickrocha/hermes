use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson, UnauthorizedErrorJson,
};
use crate::endpoints::json::inspection_model_json::InspectionModelJson;
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::{can_create_preventive_plan, can_read_preventive_plan};
use business::domain::inspection_model::InspectionModel;
use business::domain::inspection_model_item::InspectionModelItem;
use business::domain::user::User;
use business::gateway::inspection_model_gateway::InspectionModelGateway;
use business::gateway::inspection_model_item_gateway::InspectionModelItemGateway;
use business::use_cases::inspection_model_use_case::{DUPLICATE_NAME, InspectionModelUseCase};

fn use_case(state: &AppState) -> InspectionModelUseCase {
    let db = state.conn.as_ref().clone();
    InspectionModelUseCase::new(InspectionModelGateway::new(db.clone()), InspectionModelItemGateway::new(db))
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        DUPLICATE_NAME => ExceptionResponse::Conflict(locale, ErrorKey::DuplicateInspectionModel),
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

fn json(model: InspectionModel, items: Vec<InspectionModelItem>) -> InspectionModelJson {
    InspectionModelJson {
        uuid: model.uuid,
        tenant_id: model.tenant_id,
        name: model.name,
        generates_work_order: model.generates_work_order,
        periodicity_days: model.periodicity_days,
        observation: model.observation,
        active: model.active,
        items: items.into_iter().map(|i| i.description).collect(),
    }
}

#[utoipa::path(
    post,
    tag = "InspectionModel",
    path = "/inspection-model",
    request_body = InspectionModelJson,
    responses(
        (status = 201, description = "The inspection model is recorded. A model with `generatesWorkOrder: false` makes the technical inspections that name it open and link no work order. **Roles:** SysAdmin (unbound; names the owning tenant); TenantOwner, Mechanic (own tenant only).", body = InspectionModelJson),
        (status = 400, description = "Blank name, no non-blank item, or a non-positive periodicity", body = BadRequestErrorJson),
        (status = 409, description = "`DuplicateInspectionModel`: this tenant already has a model with this name", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not manage inspection models", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<InspectionModelJson>,
) -> HttpResponse<(StatusCode, Json<InspectionModelJson>)> {
    if !can_create_preventive_plan(&current_user, payload.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::PreventivePlanForbidden));
    }
    let mut tenant_id = payload.tenant_id;
    if current_user.tenant_id.is_some() {
        tenant_id = current_user.tenant_id;
    }
    let model = InspectionModel {
        id: None,
        uuid: None,
        tenant_id,
        name: payload.name,
        generates_work_order: payload.generates_work_order,
        periodicity_days: payload.periodicity_days,
        observation: payload.observation,
        active: payload.active,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };
    match use_case(&state).create(model, payload.items).await {
        Ok((saved, items)) => Ok((StatusCode::CREATED, Json(json(saved, items)))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "InspectionModel",
    path = "/inspection-model/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Inspection model UUID")),
    responses(
        (status = 200, description = "The inspection model and its items. **Roles:** any caller of the model's own tenant; SysAdmin.", body = InspectionModelJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 404, description = "Model not found, **or in another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_by_uuid(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<InspectionModelJson>> {
    let (model, items) = use_case(&state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::InspectionModelNotFound))?;
    if !can_read_preventive_plan(&current_user, model.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::InspectionModelNotFound));
    }
    Ok(Json(json(model, items)))
}

#[utoipa::path(
    get,
    tag = "InspectionModel",
    path = "/inspection-model",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of the caller's own tenant's inspection models (PD-028), by name. **Roles:** any authenticated role; SysAdmin sees every tenant, everyone else only their own.", body = PageJson<InspectionModelJson>),
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
) -> HttpResponse<Json<PageJson<InspectionModelJson>>> {
    let (page, page_size) = (page_query.page(), page_query.page_size());
    match use_case(&state).find_page(page, page_size).await {
        Ok((models, total)) => Ok(Json(PageJson::new(
            models.into_iter().map(|(m, i)| json(m, i)).collect(),
            page,
            page_size,
            total,
        ))),
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}
