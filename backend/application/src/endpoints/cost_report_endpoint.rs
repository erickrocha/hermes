use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, ForbiddenErrorJson, InternalServerErrorJson, NotFoundErrorJson, UnauthorizedErrorJson,
};
use crate::endpoints::vehicle_endpoint::find_visible;
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use business::domain::authorization::can_read_cost_reports;
use business::domain::cost_report::{VehicleCostInput, compose, supplier_csv, vehicle_csv};
use business::domain::user::User;
use business::gateway::cost_report_gateway::CostReportGateway;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct PeriodQuery {
    /// First day of the period, inclusive (`YYYY-MM-DD`).
    pub from: NaiveDate,
    /// Last day of the period, inclusive.
    pub to: NaiveDate,
}

#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct SupplierQuery {
    pub supplier: String,
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CostBySourceJson {
    pub work_order_postings_cents: i64,
    pub external_services_cents: i64,
    pub fuel_cents: i64,
    pub expenses_cents: i64,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NamedAmountJson {
    /// Empty string for the "to be classified" heading (a work order with no service type).
    pub name: String,
    pub amount: i64,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VehicleCostJson {
    pub vehicle_uuid: String,
    pub plate: String,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub by_source: CostBySourceJson,
    pub total_cost_cents: i64,
    /// Work-order cost per service type, in cents; `name` empty means "to be classified".
    pub by_service_type: Vec<NamedAmountJson>,
    /// Services performed in the period, as quantities (never cost).
    pub services_performed: Vec<NamedAmountJson>,
    pub revenue_cents: i64,
    /// Revenue less cost; absent when the vehicle had no priced trip in the period.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_cents: Option<i64>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkOrderCostJson {
    pub postings_cents: i64,
    pub external_service_cents: i64,
    pub total_cents: i64,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SupplierLineJson {
    pub date: NaiveDate,
    pub invoice_number: Option<String>,
    pub service: String,
    pub work_order_type: Option<String>,
    pub plate: String,
    pub invoice_cents: i64,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SupplierClosingJson {
    pub supplier: String,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub lines: Vec<SupplierLineJson>,
    pub total_cents: i64,
}

fn gateway(state: &AppState) -> CostReportGateway {
    CostReportGateway::new(state.conn.as_ref().clone())
}

fn guard(locale: &Locale, user: &User) -> Result<(), ExceptionResponse> {
    if can_read_cost_reports(user) {
        Ok(())
    } else {
        Err(ExceptionResponse::Forbidden(locale.clone(), ErrorKey::CostReportForbidden))
    }
}

fn period(locale: &Locale, from: NaiveDate, to: NaiveDate) -> Result<(), ExceptionResponse> {
    if from > to {
        return Err(ExceptionResponse::BadRequest(locale.clone(), ErrorKey::InvalidParameterValue));
    }
    Ok(())
}

fn internal(locale: &Locale) -> ExceptionResponse {
    ExceptionResponse::InternalServerError(locale.clone(), ErrorKey::UnexpectedError)
}

fn csv(filename: String, body: String) -> Response {
    let disposition = format!("attachment; filename=\"{}\"", filename.replace(['"', '\\', '/'], "_"));
    (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("text/csv; charset=utf-8")),
            (header::CONTENT_DISPOSITION, HeaderValue::from_str(&disposition).unwrap_or_else(|_| HeaderValue::from_static("attachment"))),
        ],
        body,
    )
        .into_response()
}

async fn vehicle_input(
    state: &AppState,
    locale: &Locale,
    user: &User,
    uuid: String,
    q: &PeriodQuery,
) -> Result<(String, String, VehicleCostInput), ExceptionResponse> {
    guard(locale, user)?;
    period(locale, q.from, q.to)?;
    let vehicle = find_visible(state, locale, user, uuid).await?;
    let gw = gateway(state);
    let input = gw
        .vehicle_input(vehicle.id.unwrap_or_default(), q.from, q.to)
        .await
        .map_err(|_| internal(locale))?;
    Ok((vehicle.uuid.unwrap_or_default(), vehicle.plate, input))
}

#[utoipa::path(
    get,
    tag = "CostReport",
    path = "/cost-report/vehicle/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Vehicle UUID"), PeriodQuery),
    responses(
        (status = 200, description = "A vehicle's cost over the period by source (work-order postings, external-service invoices, fuellings not deleted, direct expenses by competence month), the work-order cost by service type with an empty name for \"to be classified\", the services performed as quantities, the freight revenue of its non-cancelled trips and, only when it had a priced trip, the result (HRMS-804…807). A work order is dated by its conclusion, else its opening; a cancelled one is not counted. **Roles:** SysAdmin, TenantOwner, TenantUser (own tenant only).", body = VehicleCostJson),
        (status = 400, description = "`from` is after `to`", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 404, description = "No such vehicle, or it belongs to another tenant", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn vehicle_cost(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Query(q): Query<PeriodQuery>,
) -> HttpResponse<Json<VehicleCostJson>> {
    let (vehicle_uuid, plate, input) = vehicle_input(&state, &locale, &current_user, uuid, &q).await?;
    let cost = compose(&input);
    let named = |rows: Vec<(String, i64)>| rows.into_iter().map(|(name, amount)| NamedAmountJson { name, amount }).collect();
    Ok(Json(VehicleCostJson {
        vehicle_uuid,
        plate,
        from: q.from,
        to: q.to,
        by_source: CostBySourceJson {
            work_order_postings_cents: cost.by_source.work_order_postings_cents,
            external_services_cents: cost.by_source.external_services_cents,
            fuel_cents: cost.by_source.fuel_cents,
            expenses_cents: cost.by_source.expenses_cents,
        },
        total_cost_cents: cost.total_cost_cents,
        by_service_type: named(cost.by_service_type),
        services_performed: named(cost.services_performed),
        revenue_cents: cost.revenue_cents,
        result_cents: cost.result_cents,
    }))
}

#[utoipa::path(
    get,
    tag = "CostReport",
    path = "/cost-report/vehicle/uuid/{uuid}/export",
    params(("uuid" = String, Path, description = "Vehicle UUID"), PeriodQuery),
    responses(
        (status = 200, description = "The vehicle's expenses for the period as CSV (UTF-8 with BOM), one block per cost source with a subtotal, named for the vehicle and the period (HRMS-809). **Roles:** SysAdmin, TenantOwner, TenantUser (own tenant only).", content_type = "text/csv", body = String),
        (status = 400, description = "`from` is after `to`", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 404, description = "No such vehicle, or it belongs to another tenant", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn vehicle_cost_export(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
    Query(q): Query<PeriodQuery>,
) -> HttpResponse<Response> {
    let (_, plate, input) = vehicle_input(&state, &locale, &current_user, uuid, &q).await?;
    Ok(csv(format!("vehicle-expenses_{plate}_{}_{}.csv", q.from, q.to), vehicle_csv(&plate, q.from, q.to, &input)))
}

#[utoipa::path(
    get,
    tag = "CostReport",
    path = "/cost-report/work-order/uuid/{uuid}",
    params(("uuid" = String, Path, description = "Work order UUID")),
    responses(
        (status = 200, description = "A work order's total cost: its postings plus its external-service invoice value (HRMS-808). **Roles:** SysAdmin, TenantOwner, TenantUser (own tenant only).", body = WorkOrderCostJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 404, description = "No such work order, or it belongs to another tenant", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn work_order_cost(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Path(uuid): Path<String>,
) -> HttpResponse<Json<WorkOrderCostJson>> {
    guard(&locale, &current_user)?;
    match gateway(&state).work_order_cost(uuid).await {
        Ok(Some((postings, external))) => Ok(Json(WorkOrderCostJson {
            postings_cents: postings,
            external_service_cents: external,
            total_cents: postings + external,
        })),
        Ok(None) => Err(ExceptionResponse::NotFound(locale, ErrorKey::WorkOrderNotFound)),
        Err(_) => Err(internal(&locale)),
    }
}

#[utoipa::path(
    get,
    tag = "CostReport",
    path = "/cost-report/suppliers",
    responses(
        (status = 200, description = "The supplier names actually used on the tenant's external-service work orders, sorted, one per name ignoring case (HRMS-810). **Roles:** SysAdmin, TenantOwner, TenantUser (own tenant only).", body = Vec<String>),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn suppliers(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<Vec<String>>> {
    guard(&locale, &current_user)?;
    gateway(&state).suppliers().await.map(Json).map_err(|_| internal(&locale))
}

async fn closing_lines(
    state: &AppState,
    locale: &Locale,
    user: &User,
    q: &SupplierQuery,
) -> Result<Vec<business::domain::cost_report::SupplierLine>, ExceptionResponse> {
    guard(locale, user)?;
    period(locale, q.from, q.to)?;
    if q.supplier.trim().is_empty() {
        return Err(ExceptionResponse::BadRequest(locale.clone(), ErrorKey::RequiredParameterMissing));
    }
    gateway(state).supplier_lines(&q.supplier, q.from, q.to).await.map_err(|_| internal(locale))
}

#[utoipa::path(
    get,
    tag = "CostReport",
    path = "/cost-report/supplier-closing",
    params(SupplierQuery),
    responses(
        (status = 200, description = "The chosen supplier's external-service work orders in the period, with invoice number, service, work-order type, vehicle and invoice value -- nothing of freight, fuel or tolls (HRMS-810). A work order is dated by its conclusion, else its opening; a cancelled one is not listed. **Roles:** SysAdmin, TenantOwner, TenantUser (own tenant only).", body = SupplierClosingJson),
        (status = 400, description = "No supplier, or `from` is after `to`", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn supplier_closing(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Query(q): Query<SupplierQuery>,
) -> HttpResponse<Json<SupplierClosingJson>> {
    let lines = closing_lines(&state, &locale, &current_user, &q).await?;
    let total_cents = lines.iter().map(|l| l.invoice_cents).sum();
    Ok(Json(SupplierClosingJson {
        supplier: q.supplier.trim().to_string(),
        from: q.from,
        to: q.to,
        lines: lines
            .into_iter()
            .map(|l| SupplierLineJson {
                date: l.date,
                invoice_number: l.invoice_number,
                service: l.service,
                work_order_type: l.work_order_type,
                plate: l.plate,
                invoice_cents: l.invoice_cents,
            })
            .collect(),
        total_cents,
    }))
}

#[utoipa::path(
    get,
    tag = "CostReport",
    path = "/cost-report/supplier-closing/export",
    params(SupplierQuery),
    responses(
        (status = 200, description = "The supplier closing report as CSV (UTF-8 with BOM) (HRMS-810). **Roles:** SysAdmin, TenantOwner, TenantUser (own tenant only).", content_type = "text/csv", body = String),
        (status = 400, description = "No supplier, or `from` is after `to`", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 403, description = "Forbidden", body = ForbiddenErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn supplier_closing_export(
    state: State<AppState>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
    Query(q): Query<SupplierQuery>,
) -> HttpResponse<Response> {
    let lines = closing_lines(&state, &locale, &current_user, &q).await?;
    Ok(csv(
        format!("supplier-closing_{}_{}_{}.csv", q.supplier.trim(), q.from, q.to),
        supplier_csv(q.supplier.trim(), q.from, q.to, &lines),
    ))
}
