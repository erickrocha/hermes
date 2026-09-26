use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::holiday_json::HolidayJson;
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::infrastructure::mapper::{HolidayMapper, Mapper};
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::{can_administer_holiday, can_create_holiday, can_read_holiday};
use business::domain::user::User;
use business::gateway::holiday_gateway::HolidayGateway;
use business::use_cases::holiday_use_case::{DUPLICATE_HOLIDAY, HolidayUseCase};

fn use_case(state: &AppState) -> HolidayUseCase {
    HolidayUseCase::new(HolidayGateway::new(state.conn.as_ref().clone()))
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    if message == DUPLICATE_HOLIDAY {
        ExceptionResponse::Conflict(locale, ErrorKey::DuplicateHoliday)
    } else {
        ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)
    }
}

#[utoipa::path(
    post,
    tag = "Holiday",
    path = "/holiday",
    request_body = HolidayJson,
    responses(
        (status = 201, description = "Holiday recorded (HRMS-602). **Roles:** SysAdmin (unbound; names the owning tenant); TenantOwner (own tenant only).", body = HolidayJson),
        (status = 400, description = "Bad request", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not manage holidays", body = ForbiddenErrorJson),
        (status = 409, description = "`DuplicateHoliday`: this tenant already has a holiday recorded for that date", body = BadRequestErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<HolidayJson>,
) -> HttpResponse<(StatusCode, Json<HolidayJson>)> {
    let mut domain = HolidayMapper::domain(payload);

    if !can_create_holiday(&current_user, domain.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::HolidayForbidden,
        ));
    }

    if current_user.tenant_id.is_some() {
        domain.tenant_id = current_user.tenant_id;
    }

    match use_case(&state).create(domain).await {
        Ok(holiday) => Ok((StatusCode::CREATED, Json(HolidayMapper::json(holiday)))),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "Holiday",
    path = "/holiday",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of the caller's own tenant's holidays (PD-028), earliest first. **Roles:** SysAdmin, TenantOwner, TenantUser, Driver, Mechanic (any authenticated role; SysAdmin sees every tenant, everyone else only their own).", body = PageJson<HolidayJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_all(
    state: State<AppState>,
    Query(page_query): Query<PageQuery>,
    Extension(locale): Extension<Locale>,
    Extension(_current_user): Extension<User>,
) -> HttpResponse<Json<PageJson<HolidayJson>>> {
    let (page, page_size) = (page_query.page(), page_query.page_size());
    match use_case(&state).find_page(page, page_size).await {
        Ok((holidays, total)) => Ok(Json(PageJson::new(
            HolidayMapper::json_vec(holidays),
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

#[utoipa::path(
    delete,
    tag = "Holiday",
    path = "/holiday/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Holiday UUID")),
    responses(
        (status = 204, description = "The holiday is removed. **Roles:** SysAdmin (any tenant); TenantOwner (own tenant only)."),
        (status = 404, description = "Holiday not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller may read this holiday but not remove it", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn remove(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<StatusCode> {
    let holiday = use_case(&state)
        .find_by_uuid(uuid.clone())
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::HolidayNotFound))?;

    if !can_read_holiday(&current_user, holiday.tenant_id) {
        return Err(ExceptionResponse::NotFound(locale, ErrorKey::HolidayNotFound));
    }
    if !can_administer_holiday(&current_user, holiday.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::HolidayForbidden,
        ));
    }

    match use_case(&state).remove(uuid).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err(error(locale, &e.message)),
    }
}
