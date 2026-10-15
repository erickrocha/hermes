//! `EPIC-GA-05` (`HRMS-962`): what is left to do before a departure, whether the
//! tank deserves an alert, and the order the yard works in -- `TRM-469…482`.
//! Pure functions, like the validity rule they sit on: one place decides, every
//! screen, the portal and the monitor ask it (`TRM-478`, `TRM-482`).

use crate::domain::enums::GarageServiceState;
use crate::domain::tenant_rule_setting::RuleSettings;
use crate::use_cases::garage_validity::TankFact;
use chrono::{Datelike, NaiveDate, NaiveDateTime, Weekday};

/// `TRM-469`/`470`/`471`: the indices of the services that **block** a
/// departure -- required by the catalogue's explicit attribute, present in the
/// triage, and effectively pending right now. A service with no record cannot
/// block (it is simply not in the list); a desirable one never does.
pub fn required_pending(services: &[(bool, GarageServiceState)]) -> Vec<usize> {
    services
        .iter()
        .enumerate()
        .filter(|(_, (required, effective))| *required && *effective == GarageServiceState::Pending)
        .map(|(i, _)| i)
        .collect()
}

/// `TRM-474`/`475`/`476`: the fill-the-tank alert -- visual only, never a queue
/// reorder. Raised in exactly two situations: the vehicle is going to travel
/// and the tank is below the trip cut, or the level is below the floor
/// whatever is scheduled. A reading that cannot be trusted raises nothing, and
/// a charter or recurring line alone is never "going to travel" -- the caller
/// passes only real trips.
pub fn fill_tank_alert(tank: TankFact, going_to_travel: bool, rules: &RuleSettings) -> bool {
    match tank {
        TankFact::NoUsableReading => false,
        TankFact::Percent(p) => (going_to_travel && p < rules.garage_alert_trip_percent) || p < rules.garage_alert_low_percent,
    }
}

/// `TRM-477`/`1401`: whether the tank alone justifies calling the vehicle to
/// base. A resolved fuelling service suppresses the call -- the percentage is an
/// estimate, the service record a fact -- **only above the critical floor**: at
/// or below it the vehicle is called whatever the fuelling service says.
pub fn tank_calls_vehicle(alert: bool, fuelling_resolved: bool, tank: TankFact, rules: &RuleSettings) -> bool {
    if !alert {
        return false;
    }
    let critical = matches!(tank, TankFact::Percent(p) if p <= rules.garage_alert_low_percent);
    critical || !fuelling_resolved
}

/// `TRM-481`: how far ahead the yard prepares -- tomorrow, or on Friday the
/// whole weekend. (The legacy default; a per-tenant working week is open, `U-019`.)
pub fn horizon_end(today: NaiveDate) -> NaiveDate {
    if today.weekday() == Weekday::Fri { today + chrono::Duration::days(2) } else { today + chrono::Duration::days(1) }
}

/// What the queue orders by.
#[derive(Debug, Clone)]
pub struct QueueKey {
    /// `TRM-425`: zero or none means no manual priority.
    pub manual_priority: Option<i32>,
    /// The vehicle's next departure inside the horizon.
    pub next_departure: Option<NaiveDateTime>,
    pub prefix: String,
}

/// `TRM-480`: manual priority first (lower is more urgent); then one ordering by
/// today's departure time, trips and charters mixed; then the later days of the
/// horizon; then vehicles with no departure, by prefix. Returns the indices in order.
pub fn order_queue(items: &[QueueKey], today: NaiveDate) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..items.len()).collect();
    indices.sort_by_key(|&i| {
        let item = &items[i];
        let priority = item.manual_priority.filter(|p| *p > 0);
        let departure_group = match item.next_departure {
            Some(at) if at.date() <= today => 0,
            Some(_) => 1,
            None => 2,
        };
        (priority.is_none(), priority, departure_group, item.next_departure, item.prefix.to_lowercase())
    });
    indices
}

/// `TRM-499`: how a vehicle's next departure reads on the monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Readiness {
    /// A required service is pending and the departure is inside its window.
    Alert,
    /// Something is still outstanding.
    Preparing,
    /// Nothing is outstanding.
    Ready,
}

/// `TRM-499`'s window: it differs between a trip and a recurring line.
fn within_window(at: NaiveDateTime, is_trip: bool, now: NaiveDateTime, rules: &RuleSettings) -> bool {
    let minutes = if is_trip { rules.garage_monitor_trip_window_minutes } else { rules.garage_monitor_line_window_minutes };
    at - now <= chrono::Duration::minutes(minutes as i64)
}

/// `TRM-499`: alert, preparing or ready. `outstanding` is any service still
/// effectively pending (required or not). A departure already past counts as inside.
pub fn departure_readiness(at: NaiveDateTime, is_trip: bool, now: NaiveDateTime, required_pending: bool, outstanding: bool, rules: &RuleSettings) -> Readiness {
    if required_pending && within_window(at, is_trip, now, rules) {
        Readiness::Alert
    } else if outstanding {
        Readiness::Preparing
    } else {
        Readiness::Ready
    }
}

/// `TRM-498`: only an extra trip inside the urgent window with a required
/// service pending animates -- never a recurring line.
pub fn urgent_animation(at: NaiveDateTime, is_trip: bool, now: NaiveDateTime, required_pending: bool, rules: &RuleSettings) -> bool {
    is_trip && required_pending && at - now <= chrono::Duration::minutes(rules.garage_monitor_urgent_minutes as i64)
}

/// What the monitor's queue orders by.
#[derive(Debug, Clone)]
pub struct MonitorKey {
    pub manual_priority: Option<i32>,
    /// The next departure and whether it is a real trip.
    pub next_departure: Option<(NaiveDateTime, bool)>,
    pub outstanding: bool,
    pub prefix: String,
}

/// `TRM-497`: imminent departure with work outstanding first; vehicles with
/// nothing left to do last, to free the display; then manual priority, the next
/// departure and the prefix. Returns the indices in order.
pub fn order_monitor(items: &[MonitorKey], now: NaiveDateTime, rules: &RuleSettings) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..items.len()).collect();
    indices.sort_by_key(|&i| {
        let item = &items[i];
        let imminent = item.outstanding && item.next_departure.is_some_and(|(at, trip)| within_window(at, trip, now, rules));
        let group = if imminent { 0 } else if item.outstanding { 1 } else { 2 };
        let priority = item.manual_priority.filter(|p| *p > 0);
        let at = item.next_departure.map(|(at, _)| at);
        (group, priority.is_none(), priority, at.is_none(), at, item.prefix.to_lowercase())
    });
    indices
}

/// What a monitor card orders by.
#[derive(Debug, Clone)]
pub struct CardKey {
    /// `TRM-495`: the same fill-the-tank alert every other surface uses.
    pub needs_refuel: bool,
    pub tank_percent: Option<f64>,
    pub prefix: String,
}

/// `TRM-495`/`1512`: need-to-refuel first, then the lowest tank; a vehicle with no
/// usable reading after those with one; then prefix. Returns the indices in order.
pub fn order_cards(items: &[CardKey]) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..items.len()).collect();
    indices.sort_by(|&a, &b| {
        let (x, y) = (&items[a], &items[b]);
        y.needs_refuel
            .cmp(&x.needs_refuel)
            .then(x.tank_percent.is_none().cmp(&y.tank_percent.is_none()))
            .then(x.tank_percent.partial_cmp(&y.tank_percent).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| x.prefix.to_lowercase().cmp(&y.prefix.to_lowercase()))
    });
    indices
}

/// Why an away vehicle is on the call-to-base list (`TRM-485`/`487`/`1504`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallReasons {
    /// `TRM-485`: a manual call in force -- it outranks everything computed.
    pub manual: bool,
    /// `TRM-487`: a further departure still ahead.
    pub departure: bool,
    /// `TRM-1504`: the tank alone justifies the call (`tank_calls_vehicle`).
    pub tank: bool,
}

impl CallReasons {
    pub fn any(&self) -> bool {
        self.manual || self.departure || self.tank
    }
}

/// The call list's order: manual calls first, then the next departure (earliest
/// first, none last), then prefix. Returns the indices in order.
pub fn order_calls(items: &[(CallReasons, Option<NaiveDateTime>, String)]) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..items.len()).collect();
    indices.sort_by_key(|&i| {
        let (reasons, at, prefix) = &items[i];
        (!reasons.manual, at.is_none(), *at, prefix.to_lowercase())
    });
    indices
}

/// `TRM-496`: the matrix's columns -- external services before internal ones,
/// then the catalogue's own order. `(is_internal, display_order)` per column;
/// returns the indices in order.
pub fn order_columns(columns: &[(bool, i32)]) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..columns.len()).collect();
    indices.sort_by_key(|&i| (columns[i].0, columns[i].1, i));
    indices
}

/// `TRM-1520…1525`: whether a provider-reported full tank marks the fuelling
/// service performed by itself. The fuelling must be fresh (`TRM-1523`) and newer
/// than **both** the service's last confirmation (`TRM-1525`: never override what
/// an employee decided; `TRM-1524`: mark whenever it is newer, not only when the
/// confirmation predates the floor) and the floor -- the later of the vehicle's
/// physical return and the service's forced-to-pending stamp (`TRM-1521`). The
/// triage's check-in time is deliberately not part of the floor (`TRM-1522`).
pub fn should_mark_fuelling(
    fuelled_at: NaiveDateTime,
    now: NaiveDateTime,
    last_confirmation: Option<NaiveDateTime>,
    last_arrival: Option<NaiveDateTime>,
    forced_pending_at: Option<NaiveDateTime>,
    rules: &RuleSettings,
) -> bool {
    let floor = last_arrival.into_iter().chain(forced_pending_at).max();
    now - fuelled_at <= chrono::Duration::hours(rules.garage_fuelling_freshness_hours as i64)
        && last_confirmation.is_none_or(|c| fuelled_at > c)
        && floor.is_none_or(|f| fuelled_at > f)
}

/// `TRM-415`/`422`: the tenant's operating date at this instant.
pub fn operating_date(now: NaiveDateTime, rules: &RuleSettings) -> NaiveDate {
    (now + chrono::Duration::minutes(rules.garage_utc_offset_minutes as i64)).date()
}

/// What the day boundary does to one open triage (`TRM-419…422`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rollover {
    /// Dated today (`TRM-420`: even if the vehicle already left, so the employee
    /// can still record what was left undone) -- nothing changes.
    Keep,
    /// Dated before today, vehicle still at base: re-dated to today (`TRM-421`/`422`).
    Redate,
    /// Dated before today, vehicle no longer at base: closed (`TRM-419`).
    Close,
}

pub fn rollover(attendance_date: NaiveDate, today: NaiveDate, away: bool) -> Rollover {
    match (attendance_date < today, away) {
        (false, _) => Rollover::Keep,
        (true, false) => Rollover::Redate,
        (true, true) => Rollover::Close,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> RuleSettings {
        RuleSettings::default()
    }

    fn day(d: u32, h: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, d).unwrap().and_hms_opt(h, 0, 0).unwrap()
    }

    #[test]
    fn only_a_required_service_that_is_effectively_pending_blocks_a_departure() {
        use GarageServiceState::*;
        let services = [(true, Performed), (true, Pending), (false, Pending), (true, NotNeeded), (true, Pending)];
        assert_eq!(required_pending(&services), vec![1, 4]);
        assert!(required_pending(&[(false, Pending)]).is_empty(), "a desirable service never blocks");
        assert!(required_pending(&[]).is_empty(), "a service with no record cannot block");
    }

    #[test]
    fn the_tank_alert_has_exactly_two_triggers_and_a_bad_reading_raises_none() {
        let r = rules();
        // Going to travel and below 95 %: alert. Not travelling at 60 %: none. Below 40 % always.
        assert!(fill_tank_alert(TankFact::Percent(80.0), true, &r));
        assert!(!fill_tank_alert(TankFact::Percent(80.0), false, &r), "a charter or line alone never alerts (TRM-475)");
        assert!(!fill_tank_alert(TankFact::Percent(96.0), true, &r));
        assert!(fill_tank_alert(TankFact::Percent(35.0), false, &r));
        assert!(!fill_tank_alert(TankFact::NoUsableReading, true, &r));
    }

    #[test]
    fn a_resolved_fuelling_service_suppresses_the_call_only_above_the_critical_floor() {
        let r = rules();
        let at = |p| TankFact::Percent(p);
        assert!(tank_calls_vehicle(true, false, at(70.0), &r), "alert, fuelling not resolved");
        assert!(!tank_calls_vehicle(true, true, at(70.0), &r), "the record is a fact, the percentage an estimate (TRM-477)");
        assert!(tank_calls_vehicle(true, true, at(40.0), &r), "at the critical floor the call is made whatever the service says (TRM-1401)");
        assert!(!tank_calls_vehicle(false, false, at(99.0), &r));
    }

    #[test]
    fn friday_prepares_the_whole_weekend() {
        let friday = NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
        assert_eq!(horizon_end(friday), NaiveDate::from_ymd_opt(2026, 10, 4).unwrap());
        let tuesday = NaiveDate::from_ymd_opt(2026, 9, 29).unwrap();
        assert_eq!(horizon_end(tuesday), NaiveDate::from_ymd_opt(2026, 9, 30).unwrap());
    }

    #[test]
    fn the_queue_is_manual_priority_then_todays_departures_then_later_then_none_by_prefix() {
        let today = NaiveDate::from_ymd_opt(2026, 9, 29).unwrap();
        let key = |priority: Option<i32>, departure: Option<NaiveDateTime>, prefix: &str| QueueKey {
            manual_priority: priority,
            next_departure: departure,
            prefix: prefix.into(),
        };
        let items = [
            key(None, None, "2000"),                // 0: nothing scheduled
            key(None, Some(day(29, 14)), "2100"),   // 1: today 14:00
            key(Some(2), None, "2200"),             // 2: manual priority 2
            key(None, Some(day(29, 9)), "2300"),    // 3: today 09:00
            key(Some(1), Some(day(29, 18)), "2400"),// 4: manual priority 1
            key(None, Some(day(30, 6)), "2500"),    // 5: tomorrow
            key(Some(0), None, "1900"),             // 6: a zeroed priority is none
        ];
        // priority 1, priority 2, today 09:00, today 14:00, tomorrow, then no departure: 1900 before 2000.
        assert_eq!(order_queue(&items, today), vec![4, 2, 3, 1, 5, 6, 0]);
    }

    #[test]
    fn readiness_alerts_only_inside_the_kind_of_departures_window() {
        let r = rules(); // trip 120 min, line 30 min
        let now = day(29, 8);
        let ready = |at, trip, req, out| departure_readiness(at, trip, now, req, out, &r);
        assert_eq!(ready(day(29, 9), true, true, true), Readiness::Alert, "a trip in one hour");
        assert_eq!(ready(day(29, 9), false, true, true), Readiness::Preparing, "a line in one hour is outside its 30 min window");
        assert_eq!(ready(day(29, 14), true, true, true), Readiness::Preparing);
        assert_eq!(ready(day(29, 9), true, false, true), Readiness::Preparing, "a desirable service alone never alerts");
        assert_eq!(ready(day(29, 9), true, false, false), Readiness::Ready);
        assert!(urgent_animation(day(29, 9), true, now, true, &r));
        assert!(!urgent_animation(day(29, 9), false, now, true, &r), "a line never animates (TRM-498)");
        assert!(!urgent_animation(day(29, 11), true, now, true, &r), "beyond 90 minutes");
    }

    #[test]
    fn the_monitor_puts_imminent_work_first_and_finished_vehicles_last() {
        let r = rules();
        let now = day(29, 8);
        let key = |priority: Option<i32>, at: Option<(NaiveDateTime, bool)>, outstanding, prefix: &str| MonitorKey {
            manual_priority: priority,
            next_departure: at,
            outstanding,
            prefix: prefix.into(),
        };
        let items = [
            key(None, None, false, "1000"),                     // 0: nothing left
            key(Some(1), Some((day(29, 20), true)), true, "2000"), // 1: priority, but far
            key(None, Some((day(29, 9), true)), true, "3000"),  // 2: imminent, work outstanding
            key(None, Some((day(29, 9), true)), false, "4000"), // 3: imminent but done
            key(None, Some((day(29, 15), true)), true, "5000"), // 4: later
        ];
        assert_eq!(order_monitor(&items, now, &r), vec![2, 1, 4, 3, 0]);
    }

    #[test]
    fn cards_put_refuel_first_then_the_lowest_tank_and_unreadable_tanks_last() {
        let card = |needs_refuel, tank_percent, prefix: &str| CardKey { needs_refuel, tank_percent, prefix: prefix.into() };
        let items = [
            card(false, Some(90.0), "1"),
            card(true, Some(60.0), "2"),
            card(false, None, "3"),
            card(true, Some(30.0), "4"),
            card(false, Some(50.0), "5"),
        ];
        assert_eq!(order_cards(&items), vec![3, 1, 4, 0, 2]);
    }

    #[test]
    fn the_call_list_puts_manual_calls_first_even_with_nothing_scheduled() {
        let r = |manual, departure, tank| CallReasons { manual, departure, tank };
        let items = [
            (r(false, true, false), Some(day(29, 12)), "2".to_string()),
            (r(false, false, true), None, "1".to_string()),
            (r(true, false, false), None, "9".to_string()), // manual, nothing scheduled: still listed, first
            (r(false, true, false), Some(day(29, 11)), "3".to_string()),
        ];
        assert!(items.iter().all(|(reasons, _, _)| reasons.any()));
        assert_eq!(order_calls(&items), vec![2, 3, 0, 1]);
    }

    #[test]
    fn matrix_columns_put_external_services_before_internal_ones() {
        // (is_internal, display_order)
        assert_eq!(order_columns(&[(true, 1), (false, 5), (true, 0), (false, 2)]), vec![3, 1, 2, 0]);
    }

    #[test]
    fn a_fresh_full_tank_marks_fuelling_only_when_newer_than_every_employee_decision_and_the_floor() {
        let r = rules(); // 24 h freshness
        let now = day(29, 12);
        let mark = |fuelled, confirmed, arrival, forced| should_mark_fuelling(fuelled, now, confirmed, arrival, forced, &r);
        assert!(mark(day(29, 9), None, Some(day(29, 8)), None), "filled after the return: marked");
        assert!(mark(day(29, 9), Some(day(28, 9)), Some(day(29, 8)), None), "an old expired mark does not block (TRM-1524)");
        assert!(!mark(day(29, 9), Some(day(29, 10)), None, None), "an employee decided later: untouched (TRM-1525)");
        assert!(!mark(day(29, 9), None, Some(day(29, 10)), None), "filled before the return: the return consumed it");
        assert!(!mark(day(29, 9), None, None, Some(day(29, 10))), "forced back to pending after it");
        assert!(!mark(day(27, 9), None, None, None), "older than the freshness window (TRM-1523)");
    }

    #[test]
    fn the_day_boundary_keeps_today_redates_a_vehicle_still_at_base_and_closes_one_that_left() {
        let d = |n| NaiveDate::from_ymd_opt(2026, 9, n).unwrap();
        assert_eq!(rollover(d(30), d(30), true), Rollover::Keep, "today's triage stays open though the bus left (TRM-420)");
        assert_eq!(rollover(d(28), d(30), false), Rollover::Redate, "still at base: waiting for service (TRM-421/422)");
        assert_eq!(rollover(d(29), d(30), true), Rollover::Close, "no longer at base (TRM-419)");
        // The operating day is São Paulo's: 01:00 UTC on the 30th is still the 29th there.
        assert_eq!(operating_date(day(30, 1), &rules()), NaiveDate::from_ymd_opt(2026, 9, 29).unwrap());
    }
}
