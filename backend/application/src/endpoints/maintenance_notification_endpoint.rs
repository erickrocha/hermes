use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson, UnauthorizedErrorJson,
};
use crate::endpoints::json::maintenance_notification_json::{
    MaintenanceNotificationJson, MarkedReadJson, UnreadCountJson, UnreadQuery,
};
use crate::endpoints::json::page_json::{PageJson, PageQuery};
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use business::commons::functions::bytes_para_string;
use business::commons::gateway::Gateway;
use business::domain::authorization::can_work_maintenance;
use business::domain::maintenance_notification::MaintenanceNotification;
use business::domain::purchase_order::PurchaseOrder;
use business::domain::user::User;
use business::gateway::maintenance_notification_gateway::MaintenanceNotificationGateway;
use business::gateway::part_gateway::PartGateway;
use business::gateway::work_order_gateway::WorkOrderGateway;
use business::use_cases::maintenance_notification_use_case::MaintenanceNotificationUseCase;
use chrono::Utc;

fn use_case(state: &AppState) -> MaintenanceNotificationUseCase {
    MaintenanceNotificationUseCase::new(MaintenanceNotificationGateway::new(
        state.conn.as_ref().clone(),
    ))
}

/// The inbox is for the maintenance side -- whoever holds the notification permissions (`C-035`). Tenant
/// scoping of what they see is the gateway's.
fn ensure_may_use_inbox(locale: &Locale, user: &User) -> Result<(), ExceptionResponse> {
    if can_work_maintenance(user, user.tenant_id) {
        Ok(())
    } else {
        Err(ExceptionResponse::Forbidden(
            locale.clone(),
            ErrorKey::MaintenanceNotificationForbidden,
        ))
    }
}

fn unexpected(locale: Locale) -> ExceptionResponse {
    ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError)
}

async fn json(state: &AppState, n: MaintenanceNotification) -> MaintenanceNotificationJson {
    let db = state.conn.as_ref().clone();
    let work_order_uuid = WorkOrderGateway::new(db.clone())
        .find_by_id(n.work_order_id)
        .await
        .ok()
        .flatten()
        .map(|m| bytes_para_string(m.uuid));
    let part_uuid = PartGateway::new(db)
        .find_by_id(n.part_id)
        .await
        .ok()
        .flatten()
        .map(|m| bytes_para_string(m.uuid));
    MaintenanceNotificationJson {
        uuid: n.uuid.unwrap_or_default(),
        work_order_uuid,
        part_uuid,
        part_name: n.part_name,
        quantity: n.quantity,
        read: n.read_at.is_some(),
        read_at: n.read_at,
        created_at: n.created_at,
    }
}

/// `TRM-616`: raises the "awaited part arrived" notice for a purchase order that has just been received.
/// Best effort by design -- a failure is logged and never fails the receipt of the order.
pub async fn notify_part_arrived(state: &AppState, order: &PurchaseOrder) {
    let Some(work_order_id) = order.work_order_id else {
        return;
    };
    let part_name = match PartGateway::new(state.conn.as_ref().clone())
        .find_by_id(order.part_id)
        .await
    {
        Ok(Some(part)) => part.name,
        Ok(None) => {
            log::error!(
                "[MaintenanceNotification] part {} of a received purchase order was not found",
                order.part_id
            );
            return;
        }
        Err(e) => {
            log::error!("[MaintenanceNotification] could not read the part: {e}");
            return;
        }
    };
    let notification = MaintenanceNotification {
        id: None,
        uuid: None,
        tenant_id: order.tenant_id,
        work_order_id,
        part_id: order.part_id,
        part_name,
        quantity: order.quantity,
        read_at: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };
    if let Err(e) = use_case(state).create(notification).await {
        log::error!("[MaintenanceNotification] could not raise the notice: {}", e.message);
    }
}

#[utoipa::path(
    get,
    tag = "MaintenanceNotification",
    path = "/maintenance-notification",
    params(PageQuery, UnreadQuery),
    responses(
        (status = 200, description = "A page of the caller's own tenant's maintenance notices (TRM-616), most recent first; `?unread=true` keeps only the unread ones. **Roles:** SysAdmin (any tenant); TenantOwner, Mechanic (own tenant only).", body = PageJson<MaintenanceNotificationJson>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not see maintenance notices", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_all(
    state: State<AppState>,
    Query(page_query): Query<PageQuery>,
    Query(unread): Query<UnreadQuery>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<PageJson<MaintenanceNotificationJson>>> {
    ensure_may_use_inbox(&locale, &current_user)?;
    let (page, page_size) = (page_query.page(), page_query.page_size());
    let (items, total) = use_case(&state)
        .find_page(unread.unread.unwrap_or(false), page, page_size)
        .await
        .map_err(|_| unexpected(locale))?;
    let mut rows = Vec::with_capacity(items.len());
    for item in items {
        rows.push(json(&state, item).await);
    }
    Ok(Json(PageJson::new(rows, page, page_size, total)))
}

#[utoipa::path(
    get,
    tag = "MaintenanceNotification",
    path = "/maintenance-notification/unread-count",
    responses(
        (status = 200, description = "How many notices the caller's tenant has not read yet (the bell's counter). **Roles:** SysAdmin (any tenant); TenantOwner, Mechanic (own tenant only).", body = UnreadCountJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not see maintenance notices", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn unread_count(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<UnreadCountJson>> {
    ensure_may_use_inbox(&locale, &current_user)?;
    let count = use_case(&state)
        .count_unread()
        .await
        .map_err(|_| unexpected(locale))?;
    Ok(Json(UnreadCountJson { count }))
}

#[utoipa::path(
    put,
    tag = "MaintenanceNotification",
    path = "/maintenance-notification/uuid/{uuid}/read",
    params(("uuid" = String, Path, description = "Maintenance notification UUID")),
    responses(
        (status = 200, description = "The notice, now read. Idempotent: an already-read notice is returned unchanged. **Roles:** SysAdmin (any tenant); TenantOwner, Mechanic (own tenant only).", body = MaintenanceNotificationJson),
        (status = 404, description = "Notice not found, **or it belongs to another tenant**", body = NotFoundErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not see maintenance notices", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn mark_read(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<MaintenanceNotificationJson>> {
    ensure_may_use_inbox(&locale, &current_user)?;
    let saved = use_case(&state)
        .mark_read(uuid, Utc::now().naive_utc())
        .await
        .map_err(|_| {
            ExceptionResponse::NotFound(locale.clone(), ErrorKey::MaintenanceNotificationNotFound)
        })?;
    Ok(Json(json(&state, saved).await))
}

#[utoipa::path(
    put,
    tag = "MaintenanceNotification",
    path = "/maintenance-notification/read-all",
    responses(
        (status = 200, description = "Marks every unread notice of the caller's tenant as read; `marked` is how many changed. **Roles:** SysAdmin (any tenant); TenantOwner, Mechanic (own tenant only).", body = MarkedReadJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "The caller's role may not see maintenance notices", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn mark_all_read(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<MarkedReadJson>> {
    ensure_may_use_inbox(&locale, &current_user)?;
    let marked = use_case(&state)
        .mark_all_read(Utc::now().naive_utc())
        .await
        .map_err(|_| unexpected(locale))?;
    Ok(Json(MarkedReadJson { marked }))
}
