use crate::AppState;
use crate::commons::exception_response::{ExceptionResponse, HttpResponse};
use crate::commons::i18n::{ErrorKey, Locale};
use crate::endpoints::json::error_response_json::{InternalServerErrorJson, UnauthorizedErrorJson};
use crate::endpoints::json::schedule_json::{EffectiveScheduleJson, ScheduleItemJson};
use axum::Json;
use axum::extract::{Extension, Query, State};
use business::commons::functions::bytes_para_string;
use business::commons::gateway::Gateway;
use business::domain::user::User;
use business::gateway::*;
use business::use_cases::effective_schedule::{ItemKind, ResolvedFrom, ScheduleAlert};
use business::use_cases::effective_schedule_use_case::EffectiveScheduleUseCase;
use std::collections::HashMap;

pub fn schedule_use_case(state: &AppState) -> EffectiveScheduleUseCase {
    let db = state.conn.as_ref().clone();
    EffectiveScheduleUseCase::new(
        transport_demand_gateway::TransportDemandGateway::new(db.clone()),
        transport_demand_allocation_gateway::TransportDemandAllocationGateway::new(db.clone()),
        daily_schedule_gateway::DailyScheduleGateway::new(db.clone()),
        schedule_exception_gateway::ScheduleExceptionGateway::new(db.clone()),
        holiday_gateway::HolidayGateway::new(db.clone()),
        customer_day_off_gateway::CustomerDayOffGateway::new(db.clone()),
        extra_trip_gateway::ExtraTripGateway::new(db),
    )
}

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
pub struct ScheduleQuery {
    /// The date; defaults to today.
    pub date: Option<chrono::NaiveDate>,
}

#[utoipa::path(
    get,
    tag = "Schedule",
    path = "/schedule/effective",
    params(ScheduleQuery),
    responses(
        (status = 200, description = "The effective schedule of a date (TRM-001), computed on demand and never stored. A recurring demand is in the day only when its weekday matches (TRM-003); a one-off trip only on its date (TRM-002); no recurring line on a holiday or a client's day-off (TRM-004/005); an extra line only while an active allocation covers the date (TRM-006). The crew resolves exception, then day entry, then allocation (TRM-007), an exception being authoritative (TRM-008/009); an extra trip displaces a line that overlaps it and shares its driver or vehicle (TRM-012); a window ending at or before its start crosses midnight (TRM-013). Demands with no stated kind, or unreadable days, are counted, not guessed at. **Roles:** any authenticated role; own tenant only.", body = EffectiveScheduleJson),
        (status = 401, description = "Unauthorized", body = UnauthorizedErrorJson),
        (status = 500, description = "Internal server error", body = InternalServerErrorJson),
    ),
    security(("bearer_auth" = []))
)]
pub async fn effective(
    state: State<AppState>,
    Query(q): Query<ScheduleQuery>,
    Extension(locale): Extension<Locale>,
    Extension(_current_user): Extension<User>,
) -> HttpResponse<Json<EffectiveScheduleJson>> {
    let date = q.date.unwrap_or_else(|| chrono::Utc::now().date_naive());
    let day = schedule_use_case(&state)
        .day(date)
        .await
        .map_err(|_| ExceptionResponse::InternalServerError(locale, ErrorKey::UnexpectedError))?;

    // The schedule names demands, drivers and vehicles by id; the API names them by uuid.
    let db = state.conn.as_ref().clone();
    let demands: HashMap<i64, String> = transport_demand_gateway::TransportDemandGateway::new(db.clone())
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
                .map(|a| match a {
                    ScheduleAlert::MissingDriver => "MissingDriver",
                    ScheduleAlert::MissingVehicle => "MissingVehicle",
                }
                .to_string())
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
