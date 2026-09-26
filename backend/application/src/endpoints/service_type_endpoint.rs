use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::json::service_type_json::ServiceTypeJson;
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::{can_create_service_type, can_read_service_type};
use business::domain::service_type::ServiceType;
use business::domain::user::User;
use business::gateway::service_type_gateway::ServiceTypeGateway;
use business::use_cases::service_type_use_case::{
    CODE_REQUIRED, DUPLICATE_CODE, NAME_REQUIRED, ServiceTypeUseCase,
};

fn use_case(state: &AppState) -> ServiceTypeUseCase {
    ServiceTypeUseCase::new(ServiceTypeGateway::new(state.conn.as_ref().clone()))
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        DUPLICATE_CODE => ExceptionResponse::Conflict(locale, ErrorKey::DuplicateServiceCode),
        CODE_REQUIRED | NAME_REQUIRED => {
            ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)
        }
        _ => ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue),
    }
}

fn json(service_type: ServiceType) -> ServiceTypeJson {
    ServiceTypeJson {
        uuid: service_type.uuid,
        tenant_id: service_type.tenant_id,
        code: service_type.code,
        name: service_type.name,
        category: service_type.category,
        active: service_type.active,
    }
}

#[utoipa::path(
    post,
    tag = "ServiceType",
    path = "/service-type",
    request_body = ServiceTypeJson,
    responses(
        (status = 201, description = "The service type is recorded. **Roles:** SysAdmin (unbound; names the owning tenant); TenantOwner (own tenant only).", body = ServiceTypeJson),
        (status = 400, description = "Bad request, including a blank code or name", body = BadRequestErrorJson),
        (status = 409, description = "`DuplicateServiceCode`: this tenant already has a service type with this code", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not manage service types", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<ServiceTypeJson>,
) -> HttpResponse<(StatusCode, Json<ServiceTypeJson>)> {
    if !can_create_service_type(&current_user, payload.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::ServiceTypeForbidden,
        ));
    }

    let mut tenant_id = payload.tenant_id;
    if current_user.tenant_id.is_some() {
        tenant_id = current_user.tenant_id;
    }

    let service_type = ServiceType {
        id: None,
        uuid: None,
        tenant_id,
        code: payload.code,
        name: payload.name,
        category: payload.category,
        active: payload.active,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };

    match use_case(&state).create(service_type).await {
        Ok(saved) => Ok((StatusCode::CREATED, Json(json(saved)))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "ServiceType",
    path = "/service-type/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Service type UUID")),
    responses(
        (status = 200, description = "The service type. **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = ServiceTypeJson),
        (status = 404, description = "Service type not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
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
) -> HttpResponse<Json<ServiceTypeJson>> {
    let service_type = use_case(&state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::ServiceTypeNotFound))?;
    if !can_read_service_type(&current_user, service_type.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::ServiceTypeNotFound));
    }
    Ok(Json(json(service_type)))
}

#[utoipa::path(
    get,
    tag = "ServiceType",
    path = "/service-type",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of the caller's own tenant's service types (PD-028), by name. **Roles:** SysAdmin, TenantOwner, TenantUser, Driver, Mechanic (any authenticated role; SysAdmin sees every tenant, everyone else only their own).", body = PageJson<ServiceTypeJson>),
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
) -> HttpResponse<Json<PageJson<ServiceTypeJson>>> {
    let (page, page_size) = (page_query.page(), page_query.page_size());
    match use_case(&state).find_page(page, page_size).await {
        Ok((service_types, total)) => Ok(Json(PageJson::new(
            service_types.into_iter().map(json).collect(),
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
