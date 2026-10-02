use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::{DemandKind, ScheduleExceptionType};
use crate::gateway::customer_day_off_gateway::CustomerDayOffGateway;
use crate::gateway::daily_schedule_gateway::DailyScheduleGateway;
use crate::gateway::extra_trip_gateway::ExtraTripGateway;
use crate::gateway::holiday_gateway::HolidayGateway;
use crate::gateway::schedule_exception_gateway::ScheduleExceptionGateway;
use crate::gateway::transport_demand_allocation_gateway::TransportDemandAllocationGateway;
use crate::gateway::transport_demand_gateway::TransportDemandGateway;
use crate::use_cases::effective_schedule::{
    AllocationIn, DailyEntryIn, DayInputs, DemandIn, ExceptionIn, ItemKind, TripIn, effective_schedule, Day,
};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use sea_orm::DbErr;
use std::collections::HashSet;
use std::str::FromStr;

/// A vehicle's departure on a day of the horizon -- a real trip (`TRM-474`: only a
/// trip is "going to travel") or a line or charter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Departure {
    pub at: NaiveDateTime,
    pub is_trip: bool,
}

/// `EPIC-SC-04-S01` (`HRMS-610`): loads what a day depends on and asks the pure
/// rule. Nothing is stored (`TRM-001`).
pub struct EffectiveScheduleUseCase {
    demands: TransportDemandGateway,
    allocations: TransportDemandAllocationGateway,
    daily: DailyScheduleGateway,
    exceptions: ScheduleExceptionGateway,
    holidays: HolidayGateway,
    day_offs: CustomerDayOffGateway,
    trips: ExtraTripGateway,
}

impl EffectiveScheduleUseCase {
    #[allow(clippy::too_many_arguments)] // one gateway per fact the day reads
    pub fn new(
        demands: TransportDemandGateway,
        allocations: TransportDemandAllocationGateway,
        daily: DailyScheduleGateway,
        exceptions: ScheduleExceptionGateway,
        holidays: HolidayGateway,
        day_offs: CustomerDayOffGateway,
        trips: ExtraTripGateway,
    ) -> Self {
        Self { demands, allocations, daily, exceptions, holidays, day_offs, trips }
    }

    /// `TRM-001`: the effective schedule of one date for the caller's tenant.
    pub async fn day(&self, date: NaiveDate) -> Result<Day, BusinessError> {
        Ok(effective_schedule(date, &self.inputs(date).await?))
    }

    /// Every vehicle's departures from `from` to `to`, inclusive, earliest first --
    /// each day of the window read once, however many vehicles ask.
    pub async fn departures_by_vehicle(
        &self,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<std::collections::HashMap<i64, Vec<Departure>>, BusinessError> {
        let mut by_vehicle: std::collections::HashMap<i64, Vec<Departure>> = std::collections::HashMap::new();
        let mut date = from;
        while date <= to {
            for item in self.day(date).await?.items {
                if let (Some(vehicle_id), Some(at)) = (item.vehicle_id, item.start) {
                    by_vehicle
                        .entry(vehicle_id)
                        .or_default()
                        .push(Departure { at, is_trip: matches!(item.kind, ItemKind::ExtraTrip | ItemKind::OneOffTrip) });
                }
            }
            date += chrono::Duration::days(1);
        }
        for departures in by_vehicle.values_mut() {
            departures.sort_by_key(|d| d.at);
        }
        Ok(by_vehicle)
    }

    async fn inputs(&self, date: NaiveDate) -> Result<DayInputs, BusinessError> {
        let demands = self
            .demands
            .find_all()
            .await
            .map_err(database_error)?
            .into_iter()
            .map(|d| DemandIn {
                id: d.id,
                kind: d.demand_kind.as_deref().and_then(|k| DemandKind::from_str(k).ok()),
                customer_id: d.customer_id,
                name: d.line_name,
                shift_start: d.shift_start,
                shift_end: d.shift_end,
                days_of_week: d.days_of_week,
                specific_date: d.specific_date,
                active: d.active,
                specific_driver_id: d.specific_driver_id,
                specific_vehicle_id: d.specific_vehicle_id,
            })
            .collect();
        let allocations = self
            .allocations
            .find_all()
            .await
            .map_err(database_error)?
            .into_iter()
            .map(|a| AllocationIn {
                demand_id: a.demand_id,
                driver_id: a.driver_id,
                vehicle_id: a.vehicle_id,
                days_of_week: a.days_of_week,
                start_date: a.start_date,
                end_date: a.end_date,
                active: a.active,
            })
            .collect();
        let daily_entries = self
            .daily
            .find_by_date(date)
            .await
            .map_err(database_error)?
            .into_iter()
            .map(|e| DailyEntryIn {
                demand_id: e.demand_id,
                driver_id: e.driver_id,
                vehicle_id: e.vehicle_id,
                start: e.start_time,
                end: e.end_time,
            })
            .collect();
        let exceptions = self
            .exceptions
            .find_by_date(date)
            .await
            .map_err(database_error)?
            .into_iter()
            .filter_map(|e| {
                Some(ExceptionIn {
                    demand_id: e.demand_id,
                    kind: ScheduleExceptionType::from_str(&e.exception_type).ok()?,
                    new_driver_id: e.new_driver_id,
                    new_vehicle_id: e.new_vehicle_id,
                })
            })
            .collect();
        let trips = self
            .trips
            .find_on_date(date)
            .await
            .map_err(database_error)?
            .into_iter()
            .map(|t| TripIn {
                id: t.id,
                customer_id: t.customer_id,
                driver_id: t.driver_id,
                vehicle_id: t.vehicle_id,
                start: t.trip_date.and_time(t.start_time.unwrap_or(NaiveTime::MIN)),
                end: t.return_date.map(|d| d.and_time(t.return_time.unwrap_or(NaiveTime::MIN))),
            })
            .collect();
        let is_holiday = !self.holidays.find_by_date(date).await.map_err(database_error)?.is_empty();
        let day_off_customers: HashSet<i64> =
            self.day_offs.find_by_date(date).await.map_err(database_error)?.into_iter().map(|d| d.customer_id).collect();
        Ok(DayInputs { demands, allocations, daily_entries, exceptions, trips, is_holiday, day_off_customers })
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[EffectiveScheduleUseCase] {}", msg);
    BusinessError::new(msg)
}
