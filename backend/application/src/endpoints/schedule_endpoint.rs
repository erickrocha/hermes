use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{
    BadRequestErrorJson, InternalServerErrorJson, NotFoundErrorJson, UnauthorizedErrorJson,
};
use crate::endpoints::json::schedule_json::{EffectiveScheduleJson, ScheduleItemJson};
use axum::Json;
use axum::extract::{Extension, Query, State};
use business::commons::functions::bytes_para_string;
use business::commons::gateway::Gateway;
use business::domain::authorization::{can_access_tenant, is_unbound_sys_admin};
use business::domain::user::User;
use business::gateway::tenant_gateway::TenantGateway;
use business::use_cases::tenant_use_case::TenantUseCase;
use business::gateway::*;
use business::use_cases::effective_schedule::{ItemKind, ResolvedFrom, ScheduleAlert};
use business::use_cases::effective_schedule_use_case::EffectiveScheduleUseCase;
use std::collections::HashMap;

pub fn schedule_use_case(state: &AppState) -> EffectiveScheduleUseCase {
    let db = state.conn.as_ref().clone();
    EffectiveScheduleUseCase::new(
        effective_schedule_gateway::EffectiveScheduleDatabaseGateway::new(
            transport_demand_gateway::TransportDemandGateway::new(db.clone()),
            transport_demand_allocation_gateway::TransportDemandAllocationGateway::new(db.clone()),
            daily_schedule_gateway::DailyScheduleGateway::new(db.clone()),
            schedule_exception_gateway::ScheduleExceptionGateway::new(db.clone()),
            holiday_gateway::HolidayGateway::new(db.clone()),
            customer_day_off_gateway::CustomerDayOffGateway::new(db.clone()),
            extra_trip_gateway::ExtraTripGateway::new(db),
        ),
    )
}

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct ScheduleQuery {
    /// The date; defaults to today.
    pub date: Option<chrono::NaiveDate>,
    /// The tenant whose schedule is wanted. **Required for the platform
    /// administrator**, who belongs to no tenant and must choose one; a
    /// tenant-bound caller always gets their own and may omit it.
    pub tenant_uuid: Option<String>,
}

#[utoipa::path(
    get,
    tag = "Schedule",
    path = "/schedule/effective",
    params(ScheduleQuery),
    responses(
        (status = 200, description = "The effective schedule of a date (TRM-001), computed on demand and never stored. A recurring demand is in the day only when its weekday matches (TRM-003); a one-off trip only on its date (TRM-002); no recurring line on a holiday or a client's day-off (TRM-004/005); an extra line only while an active allocation covers the date (TRM-006). The crew resolves exception, then day entry, then allocation (TRM-007), an exception being authoritative (TRM-008/009); an extra trip displaces a line that overlaps it and shares its driver or vehicle (TRM-012); a window ending at or before its start crosses midnight (TRM-013). Demands with no stated kind, or unreadable days, are counted, not guessed at. **Roles:** any authenticated role; own tenant only.", body = EffectiveScheduleJson),
        (status = 400, description = "`TenantMustBeNamed`: the platform administrator did not name a tenant (`tenantUuid`)", body = BadRequestErrorJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 404, description = "The named tenant does not exist, or is another tenant's (PD-034)", body = NotFoundErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn effective(
    state: State<AppState>,
    Query(q): Query<ScheduleQuery>,
    Extension(locale): Extension<Locale>,
    Extension(current_user): Extension<User>,
) -> HttpResponse<Json<EffectiveScheduleJson>> {
    let date = q.date.unwrap_or_else(|| chrono::Utc::now().date_naive());
    // The schedule is one tenant's. A tenant-bound caller is already confined to theirs;
    // the platform administrator has no tenant, so must name one -- otherwise one
    // tenant's holiday would suppress another tenant's lines in the same view.
    if !is_unbound_sys_admin(&current_user) {
        if let Some(uuid) = q.tenant_uuid.as_deref() {
            let tenant = TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()))
                .find_by_uuid(uuid.to_string())
                .await
                .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::TenantNotFound))?;
            if !can_access_tenant(&current_user, tenant.id.unwrap_or_default()) {
                return Err(ExceptionResponse::NotFound(locale, ErrorKey::TenantNotFound));
            }
        }
        return compute(&state, &locale, date).await;
    }
    let Some(uuid) = q.tenant_uuid else {
        return Err(ExceptionResponse::BadRequest(locale, ErrorKey::TenantMustBeNamed));
    };
    let tenant = TenantUseCase::new(TenantGateway::new(state.conn.as_ref().clone()))
        .find_by_uuid(uuid)
        .await
        .map_err(|_| ExceptionResponse::NotFound(locale.clone(), ErrorKey::TenantNotFound))?;
    let tenant_id = tenant.id.unwrap_or_default();
    entity::audit::run_for_tenant(tenant_id, "schedule-view", compute(&state, &locale, date)).await
}

async fn compute(
    state: &AppState,
    locale: &Locale,
    date: chrono::NaiveDate,
) -> HttpResponse<Json<EffectiveScheduleJson>> {
        let day = schedule_use_case(state)
        .day(date)
        .await
        .map_err(|_| ExceptionResponse::InternalServerError(locale.clone(), ErrorKey::UnexpectedError))?;

    // The schedule names demands, drivers and vehicles by id; the API names them by uuid.
    let db = state.conn.as_ref().clone();
    let demands: HashMap<i64, String> =
        transport_demand_gateway::TransportDemandGateway::new(db.clone())
            .find_all()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|d| (d.id, bytes_para_string(d.uuid)))
            .collect();
    let vehicles: HashMap<i64, String> = vehicle_gateway::VehicleGateway::new(db.clone())
        .find_all()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|v| (v.id, bytes_para_string(v.uuid)))
        .collect();
    let drivers: HashMap<i64, String> = user_gateway::UserGateway::new(db)
        .find_all()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|u| (u.id, bytes_para_string(u.uuid)))
        .collect();

    let items = day
        .items
        .into_iter()
        .map(|i| ScheduleItemJson {
            kind: match i.kind {
                ItemKind::Line => "Line",
                ItemKind::ExtraLine => "ExtraLine",
                ItemKind::OneOffTrip => "OneOffTrip",
                ItemKind::ExtraTrip => "ExtraTrip",
            }
            .to_string(),
            demand_uuid: i.demand_id.and_then(|id| demands.get(&id).cloned()),
            name: i.name,
            start: i.start,
            end: i.end,
            driver_uuid: i.driver_id.and_then(|id| drivers.get(&id).cloned()),
            vehicle_uuid: i.vehicle_id.and_then(|id| vehicles.get(&id).cloned()),
            resolved_from: match i.resolved_from {
                ResolvedFrom::Exception => "Exception",
                ResolvedFrom::DailyEntry => "DailyEntry",
                ResolvedFrom::Allocation => "Allocation",
                ResolvedFrom::DemandDefault => "DemandDefault",
                ResolvedFrom::None => "None",
            }
            .to_string(),
            displaced_by_trip: i.displaced_by_trip,
            alerts: i
                .alerts
                .into_iter()
                .map(|a| {
                    match a {
                        ScheduleAlert::MissingDriver => "MissingDriver",
                        ScheduleAlert::MissingVehicle => "MissingVehicle",
                    }
                    .to_string()
                })
                .collect(),
        })
        .collect();
    Ok(Json(EffectiveScheduleJson {
        date,
        items,
        unclassified_demands: day.unclassified_demands,
        malformed_demands: day.malformed_demands,
    }))
}
