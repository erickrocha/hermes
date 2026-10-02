use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::checklist_template_item_json::ChecklistTemplateItemJson;
use crate::endpoints::json::checklist_template_json::ChecklistTemplateJson;
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson,
    UnauthorizedErrorJson,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use crate::infrastructure::mapper::reject_unknown_checklist_type;
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use business::domain::authorization::{can_create_checklist_template, can_read_checklist_template};
use business::domain::checklist_template::ChecklistTemplate;
use business::domain::checklist_template_item::ChecklistTemplateItem;
use business::domain::enums::ChecklistType;
use business::domain::user::User;
use business::gateway::checklist_template_gateway::ChecklistTemplateGateway;
use business::gateway::checklist_template_item_gateway::ChecklistTemplateItemGateway;
use business::use_cases::checklist_template_use_case::{
    AT_LEAST_ONE_ITEM_REQUIRED, ChecklistTemplateUseCase, ITEM_DESCRIPTION_REQUIRED, NAME_REQUIRED,
};
use std::str::FromStr;

fn use_case(state: &AppState) -> ChecklistTemplateUseCase {
    let db = state.conn.as_ref().clone();
    ChecklistTemplateUseCase::new(
        ChecklistTemplateGateway::new(db.clone()),
        ChecklistTemplateItemGateway::new(db),
    )
}

fn error(locale: Locale, message: &str) -> ExceptionResponse {
    match message {
        NAME_REQUIRED | AT_LEAST_ONE_ITEM_REQUIRED | ITEM_DESCRIPTION_REQUIRED => {
            ExceptionResponse::BadRequest(locale, ErrorKey::InvalidParameterValue)
        }
        _ => ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError),
    }
}

fn item_domain(item: ChecklistTemplateItemJson) -> ChecklistTemplateItem {
    ChecklistTemplateItem {
        id: None,
        uuid: None,
        tenant_id: None,
        checklist_template_id: 0,
        description: item.description,
        generates_work_order: item.generates_work_order,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn item_json(item: ChecklistTemplateItem) -> ChecklistTemplateItemJson {
    ChecklistTemplateItemJson {
        uuid: item.uuid,
        description: item.description,
        generates_work_order: item.generates_work_order,
    }
}

fn template_json(template: ChecklistTemplate, items: Vec<ChecklistTemplateItem>) -> ChecklistTemplateJson {
    ChecklistTemplateJson {
        uuid: template.uuid,
        tenant_id: template.tenant_id,
        name: template.name,
        checklist_type: template.checklist_type.to_string(),
        active: template.active,
        items: items.into_iter().map(item_json).collect(),
    }
}

#[utoipa::path(
    post,
    tag = "ChecklistTemplate",
    path = "/checklist-template",
    request_body = ChecklistTemplateJson,
    responses(
        (status = 201, description = "The template and its items are recorded together, all-or-nothing (HRMS-651). **Roles:** SysAdmin (unbound; names the owning tenant); TenantOwner (own tenant only).", body = ChecklistTemplateJson),
        (status = 400, description = "Bad request, including a checklist type outside Departure/Return/Standalone, a blank name, or no items", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not manage checklist templates", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn add(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Json(payload): Json<ChecklistTemplateJson>,
) -> HttpResponse<(StatusCode, Json<ChecklistTemplateJson>)> {
    if !can_create_checklist_template(&current_user, payload.tenant_id) {
        return Err(ExceptionResponse::Forbidden(
            locale,
            ErrorKey::ChecklistTemplateForbidden,
        ));
    }
    reject_unknown_checklist_type(&payload.checklist_type, &locale)?;

    let mut tenant_id = payload.tenant_id;
    if current_user.tenant_id.is_some() {
        tenant_id = current_user.tenant_id;
    }

    let template = ChecklistTemplate {
        id: None,
        uuid: None,
        tenant_id,
        name: payload.name,
        checklist_type: ChecklistType::from_str(&payload.checklist_type)
            .unwrap_or(ChecklistType::Standalone),
        active: payload.active,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };
    let items = payload.items.into_iter().map(item_domain).collect();

    match use_case(&state).create(template, items).await {
        Ok((saved_template, saved_items)) => Ok((
            StatusCode::CREATED,
            Json(template_json(saved_template, saved_items)),
        )),
        Err(e) => Err(error(locale, &e.message)),
    }
}

#[utoipa::path(
    get,
    tag = "ChecklistTemplate",
    path = "/checklist-template/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Checklist template UUID")),
    responses(
        (status = 200, description = "The template with its items. **Roles:** SysAdmin (any tenant); TenantOwner, TenantUser, Driver, Mechanic (own tenant only).", body = ChecklistTemplateJson),
        (status = 404, description = "Template not found, **or it belongs to another tenant** (PD-034)", body = NotFoundErrorJson),
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
) -> HttpResponse<Json<ChecklistTemplateJson>> {
    let (template, items) = use_case(&state)
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::ChecklistTemplateNotFound))?;

    if !can_read_checklist_template(&current_user, template.tenant_id) {
        return Err(ExceptionResponse::NotFound(
            locale,
            ErrorKey::ChecklistTemplateNotFound,
        ));
    }

    Ok(Json(template_json(template, items)))
}

#[utoipa::path(
    get,
    tag = "ChecklistTemplate",
    path = "/checklist-template",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of the caller's own tenant's templates (PD-028), most recently created first. Each row's `items` is empty -- read a template by uuid to see its items. **Roles:** SysAdmin, TenantOwner, TenantUser, Driver, Mechanic (any authenticated role; SysAdmin sees every tenant, everyone else only their own).", body = PageJson<ChecklistTemplateJson>),
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
) -> HttpResponse<Json<PageJson<ChecklistTemplateJson>>> {
    let (page, page_size) = (page_query.page(), page_query.page_size());
    match use_case(&state).find_page(page, page_size).await {
        Ok((templates, total)) => {
            let rows = templates
                .into_iter()
                .map(|t| template_json(t, Vec::new()))
                .collect();
            Ok(Json(PageJson::new(rows, page, page_size, total)))
        }
        Err(_) => Err(ExceptionResponse::InternalServerError(
            locale,
            ErrorKey::UnexpectedError,
        )),
    }
}
