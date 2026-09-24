use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::business_plan_json::{
    BusinessPlanJson, CreateBusinessPlanJson, UpdateBusinessPlanJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, NotFoundErrorJson, UnauthorizedErrorJson,
};
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::can_manage_business_plan_catalogue;
use business::domain::business_plan::BusinessPlan;
use business::domain::user::User;
use business::gateway::business_plan_gateway::BusinessPlanGateway;
use business::use_cases::business_plan_use_case::BusinessPlanUseCase;

fn authorize(user: &User, locale: &Locale) -> Result<(), ExceptionResponse> {
    if !can_manage_business_plan_catalogue(user) {
        return Err(ExceptionResponse::Forbidden(
            locale.clone(),
            ErrorKey::BusinessPlanForbidden,
        ));
    }
    Ok(())
}

fn use_case(state: &AppState) -> BusinessPlanUseCase {
    BusinessPlanUseCase::new(BusinessPlanGateway::new(state.conn.as_ref().clone()))
}

pub(crate) fn response(plan: BusinessPlan) -> BusinessPlanJson {
    BusinessPlanJson {
        id: plan.id.expect("persisted business plan has an id"),
        uuid: plan.uuid.expect("persisted business plan has a uuid"),
        name: plan.name,
        price_in_cents: plan.price_in_cents,
        available_users: plan.available_users,
        period_days: plan.period_days,
        payment_date: plan.payment_date,
        created_at: plan
            .created_at
            .expect("persisted business plan has created_at"),
        created_by: plan.created_by,
        updated_at: plan
            .updated_at
            .expect("persisted business plan has updated_at"),
        updated_by: plan.updated_by,
    }
}

fn domain(payload: CreateBusinessPlanJson) -> BusinessPlan {
    BusinessPlan {
        id: None,
        uuid: None,
        name: payload.name,
        price_in_cents: payload.price_in_cents,
        available_users: payload.available_users,
        period_days: payload.period_days,
        payment_date: payload.payment_date,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn update_domain(payload: UpdateBusinessPlanJson) -> BusinessPlan {
    domain(CreateBusinessPlanJson {
        name: payload.name,
        price_in_cents: payload.price_in_cents,
        available_users: payload.available_users,
        period_days: payload.period_days,
        payment_date: payload.payment_date,
    })
}

fn map_error(locale: Locale, message: &str) -> ExceptionResponse {
    if message.contains("not found") {
        ExceptionResponse::NotFound(locale, ErrorKey::BusinessPlanNotFound)
    } else if message.contains("still assigned") {
        ExceptionResponse::Conflict(locale, ErrorKey::BusinessPlanInUse)
    } else {
        ExceptionResponse::BadRequest(locale, ErrorKey::BusinessPlanInvalid)
    }
}

#[utoipa::path(
    post,
    tag = "Business Plan",
    path = "/business-plan",
    request_body = CreateBusinessPlanJson,
    responses(
        (status = 201, description = "Business plan created", body = BusinessPlanJson),
        (status = 400, body = BadRequestErrorJson),
        (status = 401, body = UnauthorizedErrorJson),
        (status = 403, body = ForbiddenErrorJson)
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    State(state): State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(user): Extension<User>,
    Json(payload): Json<CreateBusinessPlanJson>,
) -> HttpResponse<(StatusCode, Json<BusinessPlanJson>)> {
    authorize(&user, &locale)?;
    use_case(&state)
        .create(domain(payload))
        .await
        .map(|plan| (StatusCode::CREATED, Json(response(plan))))
        .map_err(|error| map_error(locale, &error.message))
}

#[utoipa::path(
    get,
    tag = "Business Plan",
    path = "/business-plan",
    params(PageQuery),
    responses(
        (status = 200, body = PageJson<BusinessPlanJson>),
        (status = 401, body = UnauthorizedErrorJson),
        (status = 403, body = ForbiddenErrorJson)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_all(
    State(state): State<AppState>,
    Query(page_query): Query<PageQuery>,
    Extension(locale): Extension<Locale>,
    Extension(user): Extension<User>,
) -> HttpResponse<Json<PageJson<BusinessPlanJson>>> {
    authorize(&user, &locale)?;
    let (page, page_size) = (page_query.page(), page_query.page_size());
    let search = page_query.search();
    use_case(&state)
        .find_page(page, page_size, search.as_deref())
        .await
        .map(|(plans, total)| {
            Json(PageJson::new(
                plans.into_iter().map(response).collect(),
                page,
                page_size,
                total,
            ))
        })
        .map_err(|error| map_error(locale, &error.message))
}

#[utoipa::path(
    get,
    tag = "Business Plan",
    path = "/business-plan/uuid/{uuid}",
    params(("uuid" = String, Path)),
    responses(
        (status = 200, body = BusinessPlanJson),
        (status = 404, body = NotFoundErrorJson),
        (status = 401, body = UnauthorizedErrorJson),
        (status = 403, body = ForbiddenErrorJson)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_by_uuid(
    State(state): State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<BusinessPlanJson>> {
    authorize(&user, &locale)?;
    use_case(&state)
        .find_by_uuid(&uuid)
        .await
        .map(|plan| Json(response(plan)))
        .map_err(|error| map_error(locale, &error.message))
}

/// HRMS-204/AD-010 (OBS-TP-05): a plan is named by its UUID, so no public URL
/// carries the sequential id. The internal id stays the database key; this
/// helper is the single place the two are bridged, so an unknown plan answers
/// 404 identically on every operation.
async fn resolve_uuid(
    state: &AppState,
    locale: &Locale,
    uuid: &str,
) -> Result<i64, ExceptionResponse> {
    let plan = use_case(state)
        .find_by_uuid(uuid)
        .await
        .map_err(|error| map_error(locale.clone(), &error.message))?;
    Ok(plan.id.unwrap_or_default())
}

#[utoipa::path(
    put,
    tag = "Business Plan",
    path = "/business-plan/uuid/{uuid}",
    params(("uuid" = String, Path)),
    request_body = UpdateBusinessPlanJson,
    responses(
        (status = 200, body = BusinessPlanJson),
        (status = 400, body = BadRequestErrorJson),
        (status = 404, body = NotFoundErrorJson),
        (status = 401, body = UnauthorizedErrorJson),
        (status = 403, body = ForbiddenErrorJson)
    ),
    security(("bearer_auth" = []))
)]
pub async fn update(
    State(state): State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(user): Extension<User>,
    Path(uuid): Path<String>,
    Json(payload): Json<UpdateBusinessPlanJson>,
) -> HttpResponse<Json<BusinessPlanJson>> {
    authorize(&user, &locale)?;
    let id = resolve_uuid(&state, &locale, &uuid).await?;
    use_case(&state)
        .update(id, update_domain(payload))
        .await
        .map(|plan| Json(response(plan)))
        .map_err(|error| map_error(locale, &error.message))
}

#[utoipa::path(
    delete,
    tag = "Business Plan",
    path = "/business-plan/uuid/{uuid}",
    params(("uuid" = String, Path)),
    responses(
        (status = 204, description = "Business plan deleted"),
        (status = 404, body = NotFoundErrorJson),
        (status = 401, body = UnauthorizedErrorJson),
        (status = 403, body = ForbiddenErrorJson)
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete(
    State(state): State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<StatusCode> {
    authorize(&user, &locale)?;
    let id = resolve_uuid(&state, &locale, &uuid).await?;
    use_case(&state)
        .delete(id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|error| map_error(locale, &error.message))
}
