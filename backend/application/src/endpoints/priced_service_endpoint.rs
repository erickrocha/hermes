use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::json::priced_service_json::PricedServiceJson;
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::{can_create_priced_service, can_read_priced_service};
use business::domain::priced_service::PricedService;
use business::domain::user::User;
use business::gateway::priced_service_gateway::PricedServiceGateway;
use business::use_cases::priced_service_use_case::{NAME_REQUIRED, PricedServiceUseCase};

fn use_case(state: &AppState) -> PricedServiceUseCase {
    PricedServiceUseCase::new(PricedServiceGateway::new(state.conn.as_ref().clone()))
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        NAME_REQUIRED => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

fn json(priced_service: PricedService) -> PricedServiceJson {
    PricedServiceJson {
        uuid: priced_service.uuid,
        tenant_id: priced_service.tenant_id,
        name: priced_service.name,
        category: priced_service.category,
        default_value_cents: priced_service.default_value_cents,
        observation: priced_service.observation,
    }
}

#[utoipa::path(
    post,
    tag = "PricedService",
    path = "/priced-service",
    request_body = PricedServiceJson,
    responses(
        (status = 201, description = "The priced service is recorded. **Roles:** SysAdmin (unbound; names the owning tenant); TenantOwner (own tenant only).", body = PricedServiceJson),
        (status = 400, description = "Bad request, including a blank name", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not manage priced services", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<PricedServiceJson>,
) -> HttpResponse<(StatusCode, Json<PricedServiceJson>)> {
    if !can_create_priced_service(&current_user, payload.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::PricedServiceForbidden,
        ));
    }

    let mut tenant_id = payload.tenant_id;
    if current_user.tenant_id.is_some() {
        tenant_id = current_user.tenant_id;
    }

    let priced_service = PricedService {
        id: None,
        uuid: None,
        tenant_id,
        name: payload.name,
        category: payload.category,
        default_value_cents: payload.default_value_cents,
        observation: payload.observation,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };

    match use_case(&state).create(priced_service).await {
        Ok(saved) => Ok((StatusCode::CREATED, Json(json(saved)))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "PricedService",
    path = "/priced-service/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Priced service UUID")),
    responses(
        (status = 200, description = "The priced service. **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = PricedServiceJson),
        (status = 404, description = "Priced service not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
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
) -> HttpResponse<Json<PricedServiceJson>> {
    let priced_service = use_case(&state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::PricedServiceNotFound))?;
    if !can_read_priced_service(&current_user, priced_service.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::PricedServiceNotFound));
    }
    Ok(Json(json(priced_service)))
}

#[utoipa::path(
    get,
    tag = "PricedService",
    path = "/priced-service",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of the caller's own tenant's priced services (PD-028), by name. **Roles:** SysAdmin, TenantOwner, TenantUser, Driver, Mechanic (any authenticated role; SysAdmin sees every tenant, everyone else only their own).", body = PageJson<PricedServiceJson>),
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
) -> HttpResponse<Json<PageJson<PricedServiceJson>>> {
    let (page, page_size) = (page_query.page(), page_query.page_size());
    match use_case(&state).find_page(page, page_size).await {
        Ok((priced_services, total)) => Ok(Json(PageJson::new(
            priced_services.into_iter().map(json).collect(),
            page,
            page_size,
            total,
        ))),
        Err(_) => Err(ExceptionResponse::InternalServerError(
            locale,
            ErrorKey::UnexpectedError,
        )),
    }
}
