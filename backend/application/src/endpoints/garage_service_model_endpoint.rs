use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson, UnauthorizedErrorJson,
};
use crate::endpoints::json::garage_service_model_json::GarageServiceModelJson;
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::{can_manage_garage_catalogue, can_read_garage};
use business::domain::enums::GarageServiceGroup;
use business::domain::garage_service_model::GarageServiceModel;
use business::domain::user::User;
use business::gateway::garage_service_model_gateway::GarageServiceModelGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::use_cases::garage_service_model_use_case::{DUPLICATE_NAME, GarageServiceModelUseCase};
use std::str::FromStr;

fn use_case(state: &AppState) -> GarageServiceModelUseCase {
    GarageServiceModelUseCase::new(
        GarageServiceModelGateway::new(state.conn.as_ref().clone()),
        VehicleGateway::new(state.conn.as_ref().clone()),
    )
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        DUPLICATE_NAME => ExceptionResponse::Conflict(locale, ErrorKey::DuplicateGarageService),
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

fn json(m: GarageServiceModel) -> GarageServiceModelJson {
    GarageServiceModelJson {
        uuid: m.uuid,
        tenant_id: m.tenant_id,
        name: m.name,
        display_order: m.display_order,
        active: m.active,
        service_group: m.service_group.to_string(),
        required_for_departure: m.required_for_departure,
        governed_by_tank: m.governed_by_tank,
        applicability: m.applicability,
    }
}

/// `TRM-431`: the group is stated, and an unknown one is refused, never guessed.
fn domain(
    locale: &Locale,
    payload: GarageServiceModelJson,
    tenant_id: Option<i64>,
    uuid: Option<String>,
) -> Result<GarageServiceModel, ExceptionResponse> {
    let service_group = GarageServiceGroup::from_str(&payload.service_group)
        .map_err(|_| ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue))?;
    Ok(GarageServiceModel {
        id: None,
        uuid,
        tenant_id,
        name: payload.name,
        name_key: String::new(),
        display_order: payload.display_order,
        active: payload.active,
        service_group,
        required_for_departure: payload.required_for_departure,
        governed_by_tank: payload.governed_by_tank,
        applicability: payload.applicability,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    })
}

#[utoipa::path(
    post,
    tag = "GarageService",
    path = "/garage-service",
    request_body = GarageServiceModelJson,
    responses(
        (status = 201, description = "The garage service is added to the tenant's catalogue (TRM-430). The group must be `External` or `Internal` (TRM-431); `requiredForDeparture` is an explicit attribute (TRM-470). `applicability` may be `all`, `vehicleTypes`, or `vehicles` (TRM-432). **Roles:** SysAdmin (unbound; names the owning tenant); TenantOwner (own tenant only).", body = GarageServiceModelJson),
        (status = 400, description = "Blank name, invalid applicability values, or a group other than External/Internal", body = BadRequestErrorJson),
        (status = 409, description = "`DuplicateGarageService`: the tenant already has a service with this normalised name (TRM-433)", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not manage the garage catalogue", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<GarageServiceModelJson>,
) -> HttpResponse<(StatusCode, Json<GarageServiceModelJson>)> {
    if !can_manage_garage_catalogue(&current_user, payload.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::GarageForbidden));
    }
    let tenant_id = current_user.tenant_id.or(payload.tenant_id);
    let model = domain(&locale, payload, tenant_id, None)?;
    match use_case(&state).create(model).await {
        Ok(saved) => Ok((StatusCode::CREATED, Json(json(saved)))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    put,
    tag = "GarageService",
    path = "/garage-service/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Garage service UUID")),
    request_body = GarageServiceModelJson,
    responses(
        (status = 200, description = "The service's name, order, active flag, group, required flag and applicability are replaced. **Roles:** SysAdmin, TenantOwner (own tenant only).", body = GarageServiceModelJson),
        (status = 400, description = "Blank name, invalid applicability values, or a group other than External/Internal", body = BadRequestErrorJson),
        (status = 409, description = "`DuplicateGarageService`: another service already has the new normalised name", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not manage the garage catalogue", body = ForbiddenErrorJson),
        (status = 404, description = "Service not found, **or in another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn update(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<GarageServiceModelJson>,
) -> HttpResponse<Json<GarageServiceModelJson>> {
    let current = use_case(&state)
        .find_by_uuid(uuid.clone())
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::GarageServiceNotFound))?;
    if !can_read_garage(&current_user, current.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::GarageServiceNotFound));
    }
    if !can_manage_garage_catalogue(&current_user, current.tenant_id) {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::GarageForbidden));
    }
    let edited = domain(&locale, payload, current.tenant_id, Some(uuid))?;
    match use_case(&state).update(edited).await {
        Ok(saved) => Ok(Json(json(saved))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "GarageService",
    path = "/garage-service/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Garage service UUID")),
    responses(
        (status = 200, description = "The garage service. **Roles:** any caller of the service's own tenant; SysAdmin.", body = GarageServiceModelJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 404, description = "Service not found, **or in another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_by_uuid(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<GarageServiceModelJson>> {
    let model = use_case(&state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::GarageServiceNotFound))?;
    if !can_read_garage(&current_user, model.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::GarageServiceNotFound));
    }
    Ok(Json(json(model)))
}

#[utoipa::path(
    get,
    tag = "GarageService",
    path = "/garage-service",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of the caller's own tenant's garage services (PD-028), in display order. **Roles:** any authenticated role; SysAdmin sees every tenant, everyone else only their own.", body = PageJson<GarageServiceModelJson>),
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
) -> HttpResponse<Json<PageJson<GarageServiceModelJson>>> {
    let (page, page_size) = (page_query.page(), page_query.page_size());
    match use_case(&state).find_page(page, page_size).await {
        Ok((models, total)) => Ok(Json(PageJson::new(models.into_iter().map(json).collect(), page, page_size, total))),
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}
