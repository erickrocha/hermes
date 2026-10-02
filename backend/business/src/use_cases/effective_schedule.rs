//! `EPIC-SC-04-S01` (`HRMS-610`, `C-024`): the **effective schedule** of a day --
//! `TRM-001…009`, `TRM-012`, `TRM-013`. Computed on demand from the demands, the
//! recurring allocations, the manual day entries, the day exceptions and the
//! extra trips, and **never stored** (`TRM-001`). A pure function over facts the
//! caller has loaded, so every surface -- the schedule screen, the garage queue,
//! the monitor -- gets the same answer.

use crate::domain::enums::{DemandKind, ScheduleExceptionType};
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Weekday};
use std::collections::HashSet;

/// `days_of_week` had no stated encoding. The owner-facing one: a comma- or
/// space-separated list of weekdays -- English (`Mon`, `Monday`), Portuguese
/// (`seg`, `segunda`) or numbers (`1`=Monday … `6`=Saturday, `0` or `7`=Sunday,
/// which the two common numberings agree on). Returns `None` if any token is
/// not a weekday, or there is none: a malformed list is refused at write and
/// reported at read, never guessed at.
pub fn parse_days_of_week(text: &str) -> Option<Vec<Weekday>> {
    let mut days = Vec::new();
    for token in text.split(|c: char| c == ',' || c == ';' || c.is_whitespace()).filter(|t| !t.is_empty()) {
        let day = match token.to_lowercase().as_str() {
            "1" | "mon" | "monday" | "seg" | "segunda" => Weekday::Mon,
            "2" | "tue" | "tuesday" | "ter" | "terca" | "terça" => Weekday::Tue,
            "3" | "wed" | "wednesday" | "qua" | "quarta" => Weekday::Wed,
            "4" | "thu" | "thursday" | "qui" | "quinta" => Weekday::Thu,
            "5" | "fri" | "friday" | "sex" | "sexta" => Weekday::Fri,
            "6" | "sat" | "saturday" | "sab" | "sáb" | "sabado" | "sábado" => Weekday::Sat,
            "0" | "7" | "sun" | "sunday" | "dom" | "domingo" => Weekday::Sun,
            _ => return None,
        };
        if !days.contains(&day) {
            days.push(day);
        }
    }
    if days.is_empty() { None } else { Some(days) }
}

#[derive(Debug, Clone)]
pub struct DemandIn {
    pub id: i64,
    pub kind: Option<DemandKind>,
    pub customer_id: Option<i64>,
    pub name: Option<String>,
    pub shift_start: Option<NaiveTime>,
    pub shift_end: Option<NaiveTime>,
    pub days_of_week: Option<String>,
    pub specific_date: Option<NaiveDate>,
    pub active: bool,
    pub specific_driver_id: Option<i64>,
    pub specific_vehicle_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct AllocationIn {
    pub demand_id: i64,
    pub driver_id: i64,
    pub vehicle_id: i64,
    pub days_of_week: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct DailyEntryIn {
    pub demand_id: i64,
    pub driver_id: i64,
    pub vehicle_id: i64,
    pub start: Option<NaiveTime>,
    pub end: Option<NaiveTime>,
}

#[derive(Debug, Clone)]
pub struct ExceptionIn {
    pub demand_id: i64,
    pub kind: ScheduleExceptionType,
    pub new_driver_id: Option<i64>,
    pub new_vehicle_id: Option<i64>,
}

/// An extra trip on the day (`EPIC-SC-03`).
#[derive(Debug, Clone)]
pub struct TripIn {
    pub id: i64,
    pub customer_id: Option<i64>,
    pub driver_id: Option<i64>,
    pub vehicle_id: Option<i64>,
    pub start: NaiveDateTime,
    pub end: Option<NaiveDateTime>,
}

/// Everything the day depends on, for one tenant.
#[derive(Debug, Clone, Default)]
pub struct DayInputs {
    pub demands: Vec<DemandIn>,
    pub allocations: Vec<AllocationIn>,
    pub daily_entries: Vec<DailyEntryIn>,
    pub exceptions: Vec<ExceptionIn>,
    pub trips: Vec<TripIn>,
    /// `TRM-004`: the date is a holiday.
    pub is_holiday: bool,
    /// `TRM-005`: clients with a registered day-off on the date.
    pub day_off_customers: HashSet<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ItemKind {
    Line,
    ExtraLine,
    OneOffTrip,
    ExtraTrip,
}

/// Where a demand's crew came from (`TRM-007`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResolvedFrom {
    Exception,
    DailyEntry,
    Allocation,
    /// The demand's own preferred driver/vehicle, when nothing above it names one.
    DemandDefault,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScheduleAlert {
    MissingDriver,
    MissingVehicle,
}

#[derive(Debug, Clone)]
pub struct ScheduleItem {
    pub demand_id: Option<i64>,
    pub trip_id: Option<i64>,
    pub kind: ItemKind,
    pub customer_id: Option<i64>,
    pub name: Option<String>,
    pub start: Option<NaiveDateTime>,
    pub end: Option<NaiveDateTime>,
    pub driver_id: Option<i64>,
    pub vehicle_id: Option<i64>,
    pub resolved_from: ResolvedFrom,
    /// `TRM-012`: a trip took a resource this line shares with it.
    pub displaced_by_trip: bool,
    pub alerts: Vec<ScheduleAlert>,
}

impl ScheduleItem {
    /// `TRM-010`: covered only when it raises no alert.
    pub fn covered(&self) -> bool {
        self.alerts.is_empty()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Day {
    pub items: Vec<ScheduleItem>,
    /// Demands with no stated kind: left out, and counted so they do not vanish silently.
    pub unclassified_demands: usize,
    /// Recurring demands whose `days_of_week` cannot be read: left out, and counted.
    pub malformed_demands: usize,
}

/// `TRM-001…009`, `012`, `013`: the effective schedule of `date`.
pub fn effective_schedule(date: NaiveDate, input: &DayInputs) -> Day {
    let mut day = Day::default();
    for demand in input.demands.iter().filter(|d| d.active) {
        let Some(kind) = demand.kind else {
            day.unclassified_demands += 1;
            continue;
        };
        match includes(demand, kind, date, input) {
            Inclusion::Yes => {}
            Inclusion::No => continue,
            Inclusion::Malformed => {
                day.malformed_demands += 1;
                continue;
            }
        }
        day.items.push(resolve(demand, kind, date, input));
    }
    for trip in &input.trips {
        day.items.push(ScheduleItem {
            demand_id: None,
            trip_id: Some(trip.id),
            kind: ItemKind::ExtraTrip,
            customer_id: trip.customer_id,
            name: None,
            start: Some(trip.start),
            end: trip.end,
            driver_id: trip.driver_id,
            vehicle_id: trip.vehicle_id,
            resolved_from: ResolvedFrom::None,
            displaced_by_trip: false,
            alerts: Vec::new(),
        });
    }
    displace_by_trips(&mut day.items, &input.trips, date);
    day
}

enum Inclusion {
    Yes,
    No,
    Malformed,
}

fn includes(demand: &DemandIn, kind: DemandKind, date: NaiveDate, input: &DayInputs) -> Inclusion {
    // `TRM-002`: a one-off trip belongs to its own date only, and stays on a holiday.
    if kind == DemandKind::OneOffTrip {
        return if demand.specific_date == Some(date) { Inclusion::Yes } else { Inclusion::No };
    }
    // `TRM-003`: a recurring demand is in a day only when the weekday is among its days.
    let Some(days) = demand.days_of_week.as_deref().and_then(parse_days_of_week) else {
        return Inclusion::Malformed;
    };
    if !days.contains(&date.weekday()) {
        return Inclusion::No;
    }
    // `TRM-004`/`005`: no recurring line on a holiday or on its client's day-off.
    if input.is_holiday || demand.customer_id.is_some_and(|c| input.day_off_customers.contains(&c)) {
        return Inclusion::No;
    }
    // `TRM-006`: an extra line exists only while a linked, active allocation window covers the date.
    if kind == DemandKind::ExtraLine && !input.allocations.iter().any(|a| allocation_covers(a, demand.id, date)) {
        return Inclusion::No;
    }
    Inclusion::Yes
}

fn allocation_covers(a: &AllocationIn, demand_id: i64, date: NaiveDate) -> bool {
    a.demand_id == demand_id
        && a.active
        && a.start_date <= date
        && a.end_date.is_none_or(|end| date <= end)
        && a.days_of_week.as_deref().is_none_or(|text| parse_days_of_week(text).is_some_and(|d| d.contains(&date.weekday())))
}

/// `TRM-007`/`008`/`009`: the crew, in the precedence exception, then manual day
/// entry, then the recurring allocation -- with the demand's own preferences as
/// the last resort. An exception is authoritative, including when it leaves the
/// demand without a driver or a vehicle.
fn resolve(demand: &DemandIn, kind: DemandKind, date: NaiveDate, input: &DayInputs) -> ScheduleItem {
    let daily = input.daily_entries.iter().find(|e| e.demand_id == demand.id);
    let allocation = input.allocations.iter().find(|a| allocation_covers(a, demand.id, date));
    let exception = input.exceptions.iter().find(|e| e.demand_id == demand.id);

    let (mut driver, mut vehicle, mut from) = match (daily, allocation) {
        (Some(d), _) => (Some(d.driver_id), Some(d.vehicle_id), ResolvedFrom::DailyEntry),
        (None, Some(a)) => (Some(a.driver_id), Some(a.vehicle_id), ResolvedFrom::Allocation),
        (None, None) => (demand.specific_driver_id, demand.specific_vehicle_id, ResolvedFrom::DemandDefault),
    };
    if from == ResolvedFrom::DemandDefault && driver.is_none() && vehicle.is_none() {
        from = ResolvedFrom::None;
    }
    if let Some(exception) = exception {
        match exception.kind {
            ScheduleExceptionType::Cancellation | ScheduleExceptionType::Deallocation => {
                driver = None;
                vehicle = None;
            }
            ScheduleExceptionType::Substitution => {
                driver = exception.new_driver_id.or(driver);
                vehicle = exception.new_vehicle_id.or(vehicle);
            }
        }
        from = ResolvedFrom::Exception;
    }

    let start_time = daily.and_then(|d| d.start).or(demand.shift_start);
    let end_time = daily.and_then(|d| d.end).or(demand.shift_end);
    let start = start_time.map(|t| date.and_time(t));
    // `TRM-013`: an end not after the start crosses midnight.
    let end = match (start, end_time) {
        (Some(s), Some(e)) => {
            let end = date.and_time(e);
            Some(if end <= s { end + Duration::days(1) } else { end })
        }
        _ => None,
    };

    let mut item = ScheduleItem {
        demand_id: Some(demand.id),
        trip_id: None,
        kind: match kind {
            DemandKind::Line => ItemKind::Line,
            DemandKind::ExtraLine => ItemKind::ExtraLine,
            DemandKind::OneOffTrip => ItemKind::OneOffTrip,
        },
        customer_id: demand.customer_id,
        name: demand.name.clone(),
        start,
        end,
        driver_id: driver,
        vehicle_id: vehicle,
        resolved_from: from,
        displaced_by_trip: false,
        alerts: Vec::new(),
    };
    refresh_alerts(&mut item);
    item
}

fn refresh_alerts(item: &mut ScheduleItem) {
    item.alerts.clear();
    if item.driver_id.is_none() {
        item.alerts.push(ScheduleAlert::MissingDriver);
    }
    if item.vehicle_id.is_none() {
        item.alerts.push(ScheduleAlert::MissingVehicle);
    }
}

/// `TRM-012`: an extra trip has priority over a line that overlaps it in time
/// and shares its driver or its vehicle: the resource is taken from the line,
/// which is marked uncovered and displaced.
fn displace_by_trips(items: &mut [ScheduleItem], trips: &[TripIn], date: NaiveDate) {
    let end_of_day = date.and_time(NaiveTime::MIN) + Duration::days(1);
    for item in items.iter_mut().filter(|i| i.kind != ItemKind::ExtraTrip && i.kind != ItemKind::OneOffTrip) {
        let (Some(start), Some(end)) = (item.start, item.end) else { continue };
        for trip in trips {
            let trip_end = trip.end.unwrap_or(end_of_day);
            if !(start < trip_end && trip.start < end) {
                continue;
            }
            let mut displaced = false;
            if item.vehicle_id.is_some() && item.vehicle_id == trip.vehicle_id {
                item.vehicle_id = None;
                displaced = true;
            }
            if item.driver_id.is_some() && item.driver_id == trip.driver_id {
                item.driver_id = None;
                displaced = true;
            }
            if displaced {
                item.displaced_by_trip = true;
                refresh_alerts(item);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, day).unwrap() // 2026-09-28 is a Monday
    }

    fn t(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    fn demand(id: i64, kind: Option<DemandKind>, days: Option<&str>) -> DemandIn {
        DemandIn {
            id,
            kind,
            customer_id: Some(7),
            name: Some(format!("demand {id}")),
            shift_start: Some(t(6, 0)),
            shift_end: Some(t(9, 0)),
            days_of_week: days.map(str::to_string),
            specific_date: None,
            active: true,
            specific_driver_id: None,
            specific_vehicle_id: None,
        }
    }

    fn allocation(demand_id: i64, driver: i64, vehicle: i64) -> AllocationIn {
        AllocationIn { demand_id, driver_id: driver, vehicle_id: vehicle, days_of_week: None, start_date: d(1), end_date: None, active: true }
    }

    fn inputs(demands: Vec<DemandIn>, allocations: Vec<AllocationIn>) -> DayInputs {
        DayInputs { demands, allocations, ..DayInputs::default() }
    }

    #[test]
    fn days_of_week_reads_english_portuguese_and_numbers_and_refuses_anything_else() {
        use Weekday::*;
        assert_eq!(parse_days_of_week("Mon, Wed Fri"), Some(vec![Mon, Wed, Fri]));
        assert_eq!(parse_days_of_week("seg;ter;sábado"), Some(vec![Mon, Tue, Sat]));
        assert_eq!(parse_days_of_week("1,2,3,4,5"), Some(vec![Mon, Tue, Wed, Thu, Fri]));
        assert_eq!(parse_days_of_week("0,7"), Some(vec![Sun]), "both numberings call Sunday 0 or 7");
        assert_eq!(parse_days_of_week("Mon,Funday"), None);
        assert_eq!(parse_days_of_week(""), None);
    }

    #[test]
    fn a_recurring_demand_is_in_a_day_only_when_its_weekday_matches_and_a_one_off_only_on_its_date() {
        let monday = d(28);
        let input = inputs(
            vec![
                demand(1, Some(DemandKind::Line), Some("Mon,Tue")),
                demand(2, Some(DemandKind::Line), Some("Wed")),
                DemandIn { specific_date: Some(monday), ..demand(3, Some(DemandKind::OneOffTrip), None) },
                DemandIn { specific_date: Some(d(29)), ..demand(4, Some(DemandKind::OneOffTrip), None) },
            ],
            vec![],
        );
        let ids: Vec<_> = effective_schedule(monday, &input).items.iter().filter_map(|i| i.demand_id).collect();
        assert_eq!(ids, vec![1, 3]);
    }

    #[test]
    fn a_holiday_and_a_client_day_off_remove_recurring_lines_but_not_dated_trips() {
        let monday = d(28);
        let mut input = inputs(
            vec![
                demand(1, Some(DemandKind::Line), Some("Mon")),
                DemandIn { specific_date: Some(monday), ..demand(3, Some(DemandKind::OneOffTrip), None) },
            ],
            vec![],
        );
        input.is_holiday = true;
        let ids: Vec<_> = effective_schedule(monday, &input).items.iter().filter_map(|i| i.demand_id).collect();
        assert_eq!(ids, vec![3], "TRM-004");
        input.is_holiday = false;
        input.day_off_customers.insert(7);
        let ids: Vec<_> = effective_schedule(monday, &input).items.iter().filter_map(|i| i.demand_id).collect();
        assert_eq!(ids, vec![3], "TRM-005");
    }

    #[test]
    fn an_extra_line_exists_only_while_an_active_allocation_window_covers_the_date() {
        let monday = d(28);
        let line = demand(1, Some(DemandKind::ExtraLine), Some("Mon"));
        assert!(effective_schedule(monday, &inputs(vec![line.clone()], vec![])).items.is_empty());
        let expired = AllocationIn { end_date: Some(d(10)), ..allocation(1, 5, 9) };
        assert!(effective_schedule(monday, &inputs(vec![line.clone()], vec![expired])).items.is_empty(), "TRM-006");
        assert_eq!(effective_schedule(monday, &inputs(vec![line], vec![allocation(1, 5, 9)])).items.len(), 1);
    }

    #[test]
    fn the_crew_comes_from_exception_then_day_entry_then_allocation_and_an_exception_is_authoritative() {
        let monday = d(28);
        let mut input = inputs(vec![demand(1, Some(DemandKind::Line), Some("Mon"))], vec![allocation(1, 5, 9)]);
        let crew = |input: &DayInputs| {
            let item = &effective_schedule(monday, input).items[0];
            (item.driver_id, item.vehicle_id, item.resolved_from)
        };
        assert_eq!(crew(&input), (Some(5), Some(9), ResolvedFrom::Allocation));
        input.daily_entries.push(DailyEntryIn { demand_id: 1, driver_id: 6, vehicle_id: 10, start: None, end: None });
        assert_eq!(crew(&input), (Some(6), Some(10), ResolvedFrom::DailyEntry));
        // A substitution replaces only what it names.
        input.exceptions.push(ExceptionIn { demand_id: 1, kind: ScheduleExceptionType::Substitution, new_driver_id: None, new_vehicle_id: Some(11) });
        assert_eq!(crew(&input), (Some(6), Some(11), ResolvedFrom::Exception));
        // A cancellation clears both -- deliberately leaving the demand uncovered (TRM-008/009).
        input.exceptions[0].kind = ScheduleExceptionType::Cancellation;
        assert_eq!(crew(&input), (None, None, ResolvedFrom::Exception));
        assert_eq!(effective_schedule(monday, &input).items[0].alerts, vec![ScheduleAlert::MissingDriver, ScheduleAlert::MissingVehicle]);
    }

    #[test]
    fn a_window_whose_end_is_not_after_its_start_crosses_midnight() {
        let monday = d(28);
        let night = DemandIn { shift_start: Some(t(22, 0)), shift_end: Some(t(5, 0)), ..demand(1, Some(DemandKind::Line), Some("Mon")) };
        let item = &effective_schedule(monday, &inputs(vec![night], vec![allocation(1, 5, 9)])).items[0];
        assert_eq!(item.end.unwrap(), d(29).and_time(t(5, 0)));
    }

    #[test]
    fn an_extra_trip_takes_a_shared_resource_from_an_overlapping_line_and_leaves_it_uncovered() {
        let monday = d(28);
        let mut input = inputs(vec![demand(1, Some(DemandKind::Line), Some("Mon"))], vec![allocation(1, 5, 9)]);
        // Overlaps 06:00-09:00 and shares the vehicle 9 (not the driver).
        input.trips.push(TripIn { id: 1, customer_id: None, driver_id: Some(99), vehicle_id: Some(9), start: monday.and_time(t(8, 0)), end: Some(monday.and_time(t(12, 0))) });
        let day = effective_schedule(monday, &input);
        let line = day.items.iter().find(|i| i.demand_id == Some(1)).unwrap();
        assert!(line.displaced_by_trip && line.vehicle_id.is_none() && line.driver_id == Some(5));
        assert_eq!(line.alerts, vec![ScheduleAlert::MissingVehicle]);
        // A trip that does not overlap in time displaces nothing.
        input.trips[0].start = monday.and_time(t(10, 0));
        let day = effective_schedule(monday, &input);
        assert!(!day.items.iter().find(|i| i.demand_id == Some(1)).unwrap().displaced_by_trip);
    }

    #[test]
    fn a_demand_with_no_kind_or_unreadable_days_is_counted_not_guessed_at() {
        let monday = d(28);
        let input = inputs(
            vec![demand(1, None, Some("Mon")), demand(2, Some(DemandKind::Line), Some("Funday")), demand(3, Some(DemandKind::Line), None)],
            vec![],
        );
        let day = effective_schedule(monday, &input);
        assert!(day.items.is_empty());
        assert_eq!((day.unclassified_demands, day.malformed_demands), (1, 2));
    }
}
