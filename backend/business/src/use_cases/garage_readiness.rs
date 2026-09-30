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
}
