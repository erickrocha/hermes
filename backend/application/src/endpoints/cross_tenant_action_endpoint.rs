use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::cross_tenant_action_json::CrossTenantActionJson;
use crate::endpoints::json::error_response_json::{ForbiddenErrorJson, InternalServerErrorJson, UnauthorizedErrorJson};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::infrastructure::mapper::CrossTenantActionMapper;
use axum::Json;
use axum::extract::{Extension, Query, State};
use business::domain::user::User;
use business::gateway::cross_tenant_action_gateway::CrossTenantActionGateway;
use business::use_cases::cross_tenant_action_use_case::CrossTenantActionUseCase;

#[utoipa::path(
    get,
    tag = "CrossTenantAction",
    path = "/cross-tenant-action",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of what a tenant did inside its descendants, newest first (`HRMS-877`): the actions in which the caller's tenant is the **affected** one (what the support did in its data) and those it **acted** in. Reads and writes are both recorded. **Roles:** `cross-tenant-action.read` -- the tenant owner by default.", body = PageJson<CrossTenantActionJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller may not read the cross-tenant trail", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_all(
    state: State<AppState>,
    Query(page_query): Query<PageQuery>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<PageJson<CrossTenantActionJson>>> {
    let Some(tenant_id) = current_user.tenant_id else {
        return Err(ExceptionResponse::Forbidden(locale, ErrorKey::PermissionNotInProfile));
    };
    let (page, page_size) = (page_query.page(), page_query.page_size());
    let use_case = CrossTenantActionUseCase::new(CrossTenantActionGateway::new(state.conn.as_ref().clone()));
    match use_case.find_page(tenant_id, page, page_size).await {
        Ok((actions, total)) => Ok(Json(PageJson::new(CrossTenantActionMapper::json_vec(actions), page, page_size, total))),
        Err(_) => Err(ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)),
    }
}
