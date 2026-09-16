use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::business_plan_json::{
    BusinessPlanJson, BusinessPlanTierJson, CreateBusinessPlanJson, UpdateBusinessPlanJson,
};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, NotFoundErrorJson, UnauthorizedErrorJson,
};
use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use business::domain::business_plan::BusinessPlan;
use business::domain::business_plan_tier::BusinessPlanTier;
use business::domain::enums::Role;
use business::domain::user::User;
use business::gateway::business_plan_gateway::BusinessPlanGateway;
use business::use_cases::business_plan_use_case::BusinessPlanUseCase;

fn authorize(user: &User, locale: Locale) -> Result<(), ExceptionResponse> {
    if user.role != Role::SysAdmin || user.tenant_id.is_some() {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::BusinessPlanForbidden,
        ));
    }
    Ok(())
}

fn use_case(state: &AppState) -> BusinessPlanUseCase {
    BusinessPlanUseCase::new(BusinessPlanGateway::new(state.conn.as_ref().clone()))
}

fn response(plan: BusinessPlan) -> BusinessPlanJson {
    BusinessPlanJson {
        id: plan.id.expect("persisted business plan has an id"),
        uuid: plan.uuid.expect("persisted business plan has a uuid"),
        name: plan.name,
        price_in_cents: plan.price_in_cents,
        available_users: plan.available_users,
        period_days: plan.period_days,
        payment_date: plan.payment_date,
        daily_ai_quota: plan.daily_ai_quota,
        created_at: plan
            .created_at
            .expect("persisted business plan has created_at"),
        created_by: plan.created_by,
        updated_at: plan
            .updated_at
            .expect("persisted business plan has updated_at"),
        updated_by: plan.updated_by,
        tiers: plan
            .tiers
            .into_iter()
            .map(|t| BusinessPlanTierJson {
                id: t.id,
                up_to_users: t.up_to_users,
                price_per_user_in_cents: t.price_per_user_in_cents,
            })
            .collect(),
    }
}

fn domain(payload: CreateBusinessPlanJson) -> BusinessPlan {
    let tiers = payload
        .tiers
        .unwrap_or_default()
        .into_iter()
        .map(|t| BusinessPlanTier {
            id: t.id,
            uuid: None,
            business_plan_id: 0,
            up_to_users: t.up_to_users,
            price_per_user_in_cents: t.price_per_user_in_cents,
        })
        .collect();

    BusinessPlan {
        id: None,
        uuid: None,
        name: payload.name,
        price_in_cents: payload.price_in_cents,
        available_users: payload.available_users,
        period_days: payload.period_days,
        payment_date: payload.payment_date,
        daily_ai_quota: payload.daily_ai_quota,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
        tiers,
    }
}

fn update_domain(payload: UpdateBusinessPlanJson) -> BusinessPlan {
    domain(CreateBusinessPlanJson {
        name: payload.name,
        price_in_cents: payload.price_in_cents,
        available_users: payload.available_users,
        period_days: payload.period_days,
        payment_date: payload.payment_date,
        daily_ai_quota: payload.daily_ai_quota,
        tiers: payload.tiers,
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
    authorize(&user, locale)?;
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
    responses(
        (status = 200, body = Vec<BusinessPlanJson>),
        (status = 401, body = UnauthorizedErrorJson),
        (status = 403, body = ForbiddenErrorJson)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_all(
    State(state): State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(user): Extension<User>,
) -> HttpResponse<Json<Vec<BusinessPlanJson>>> {
    authorize(&user, locale)?;
    use_case(&state)
        .find_all()
        .await
        .map(|plans| Json(plans.into_iter().map(response).collect()))
        .map_err(|error| map_error(locale, &error.message))
}

#[utoipa::path(
    get,
    tag = "Business Plan",
    path = "/business-plan/{id}",
    params(("id" = i64, Path)),
    responses(
        (status = 200, body = BusinessPlanJson),
        (status = 404, body = NotFoundErrorJson),
        (status = 401, body = UnauthorizedErrorJson),
        (status = 403, body = ForbiddenErrorJson)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_by_id(
    State(state): State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(user): Extension<User>,
    Path(id): Path<i64>,
) -> HttpResponse<Json<BusinessPlanJson>> {
    authorize(&user, locale)?;
    use_case(&state)
        .find_by_id(id)
        .await
        .map(|plan| Json(response(plan)))
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
    authorize(&user, locale)?;
    use_case(&state)
        .find_by_uuid(&uuid)
        .await
        .map(|plan| Json(response(plan)))
        .map_err(|error| map_error(locale, &error.message))
}

#[utoipa::path(
    put,
    tag = "Business Plan",
    path = "/business-plan/{id}",
    params(("id" = i64, Path)),
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
    Path(id): Path<i64>,
    Json(payload): Json<UpdateBusinessPlanJson>,
) -> HttpResponse<Json<BusinessPlanJson>> {
    authorize(&user, locale)?;
    use_case(&state)
        .update(id, update_domain(payload))
        .await
        .map(|plan| Json(response(plan)))
        .map_err(|error| map_error(locale, &error.message))
}

#[utoipa::path(
    delete,
    tag = "Business Plan",
    path = "/business-plan/{id}",
    params(("id" = i64, Path)),
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
    Path(id): Path<i64>,
) -> HttpResponse<StatusCode> {
    authorize(&user, locale)?;
    use_case(&state)
        .delete(id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|error| map_error(locale, &error.message))
}
