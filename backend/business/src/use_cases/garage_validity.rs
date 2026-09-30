//! `EPIC-GA-04` (`HRMS-961`, `C-028`): the validity of a garage service --
//! `TRM-447…468`. "The densest and most contested sub-domain in the project"
//! is, in hermes, one pure function evaluated in a fixed order and **never
//! stored** (`TRM-447`): every screen, the portal and the monitor ask it.
//!
//! What consumes a service is the vehicle's **use**, never the calendar
//! (`TRM-449`): a vehicle standing at the yard never has its preparation expire.
//! Per `D-24(b)` (the owner's recorded 2026-08-03 ruling) the 36-hour count
//! restarts on **every** departure -- the vehicle's *last* departure, not a
//! per-service first-departure anchor -- so a service record carries no anchor
//! to clear (`TRM-439`) or backfill (`TRM-492`).

use crate::domain::enums::GarageServiceState;
use crate::domain::tenant_rule_setting::RuleSettings;
use chrono::{Duration, NaiveDateTime};

/// The service's own facts.
#[derive(Debug, Clone, Copy)]
pub struct ServiceFacts {
    pub state: GarageServiceState,
    pub performed_at: Option<NaiveDateTime>,
    pub marked_at: Option<NaiveDateTime>,
    pub forced_pending_at: Option<NaiveDateTime>,
    /// `TRM-462`: an attribute of the catalogue entry, never a name match.
    pub governed_by_tank: bool,
}

/// The vehicle's physical stamps (`EPIC-GA-03`), identically for every surface (`TRM-448`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PresenceFacts {
    pub last_arrival_at: Option<NaiveDateTime>,
    pub last_departure_at: Option<NaiveDateTime>,
    /// The latest event is a departure.
    pub away: bool,
    /// The most recent completed run: the departure and the arrival that closed it.
    pub last_run: Option<(NaiveDateTime, NaiveDateTime)>,
}

/// The tank reading a tank-governed service consults (`TRM-462`, `TRM-468`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TankFact {
    /// No reading, or one that is awaiting calibration or suspect: not to be trusted.
    NoUsableReading,
    Percent(f64),
}

#[derive(Debug, Clone, Copy)]
pub struct ValidityContext {
    pub presence: PresenceFacts,
    /// `TRM-784`: when the vehicle's latest trip entered the system, which obliges
    /// its preparation to be redone -- `None` when no trip is marked for it.
    pub trip_marked_at: Option<NaiveDateTime>,
    pub tank: TankFact,
    pub now: NaiveDateTime,
}

/// Why a service is in the state it is -- the rule explains itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValidityReason {
    /// Pending is sovereign (`TRM-459`).
    Pending,
    /// A forcing-to-pending stamp at or after the performance is decisive (`TRM-460`).
    ForcedPending,
    /// No decision stamp to reason from.
    NoStamp,
    /// The vehicle departed after a not-needed/not-done mark (`TRM-457`).
    ConsumedByDeparture,
    /// The vehicle's arrival is later than the mark or the performance (`TRM-455`, `TRM-461`).
    ReturnedToBase,
    /// A return from a run that counts as a trip resets the service (`TRM-461`).
    TripReturn,
    /// A trip was marked and physical use followed the service (`TRM-465`/`466`).
    TripMarked,
    /// More than the validity window after the vehicle's last departure (`TRM-450`).
    Expired,
    /// A tank-governed service below the configured level (`TRM-462`).
    TankLow,
    /// A tank-governed service concluded, under watch of the tank (`TRM-462`).
    TankGoverned,
    /// Still valid: nothing has consumed it.
    Valid,
}

/// The effective state of one service, and why. The fixed order is the rule:
/// pending; forced pending; the stamp; then per state (marks: departure, then
/// arrival, then a marked trip; performances: a trip return, the tank or the
/// trip marker, then the time rule).
pub fn effective_state(service: &ServiceFacts, ctx: &ValidityContext, rules: &RuleSettings) -> (GarageServiceState, ValidityReason) {
    use GarageServiceState::*;
    use ValidityReason as R;

    if service.state == Pending {
        return (Pending, R::Pending);
    }
    if service.state == Performed
        && let (Some(performed), Some(forced)) = (service.performed_at, service.forced_pending_at)
        && forced >= performed
    {
        return (Pending, R::ForcedPending);
    }
    let stamp = match service.state {
        Performed => service.performed_at,
        _ => service.marked_at,
    };
    let Some(stamp) = stamp else { return (Pending, R::NoStamp) };
    let p = &ctx.presence;
    let departed_after = p.last_departure_at.is_some_and(|d| d > stamp);
    let arrived_after = p.last_arrival_at.is_some_and(|a| a > stamp);

    if service.state != Performed {
        // A not-needed / not-done mark documents what was decided for a run: valid
        // indefinitely for a vehicle that has not departed since (`TRM-456`), and
        // never expiring by time while it is away (`TRM-454`) -- but the run consumes
        // it (`TRM-457`) and the return ends it (`TRM-455`).
        if departed_after {
            return (Pending, R::ConsumedByDeparture);
        }
        if arrived_after {
            return (Pending, R::ReturnedToBase);
        }
        return match trip_consumes(stamp, service, ctx, rules) {
            true => (Pending, R::TripMarked),
            false => (service.state, R::Valid),
        };
    }

    // `TRM-461`: a return that counts as a trip resets every service at once,
    // independently of any window. (A mark made between the departure and this
    // return documents what the vehicle left with and is consumed by the run,
    // `TRM-467` -- the same condition.)
    if let Some((departed, arrived)) = p.last_run
        && arrived > stamp
        && run_counts_as_trip(departed, arrived, rules)
    {
        return (Pending, R::TripReturn);
    }
    if service.governed_by_tank
        && let TankFact::Percent(percent) = ctx.tank
    {
        if trip_consumes(stamp, service, ctx, rules) {
            return (Pending, R::TripMarked);
        }
        return if percent < rules.garage_fuel_pending_below_percent {
            (Pending, R::TankLow)
        } else {
            (Performed, R::TankGoverned)
        };
    }
    if trip_consumes(stamp, service, ctx, rules) {
        return (Pending, R::TripMarked);
    }
    // `TRM-450`/`451`: expires the window after the vehicle's last departure, and
    // never while no departure has followed the service.
    if let Some(departed) = p.last_departure_at
        && departed > stamp
        && ctx.now >= departed + Duration::hours(rules.garage_validity_hours as i64)
    {
        return (Pending, R::Expired);
    }
    (Performed, R::Valid)
}

/// `TRM-782`: an absence counts as a trip only when it lasted at least the minimum.
fn run_counts_as_trip(departed: NaiveDateTime, arrived: NaiveDateTime, rules: &RuleSettings) -> bool {
    arrived - departed >= Duration::minutes(rules.garage_min_trip_absence_minutes as i64)
}

/// `TRM-465`/`466`/`468`: a marked trip obliges every applicable service to be
/// redone -- but a scheduled trip alone never consumes the preparation of a
/// vehicle that has not moved: only proof of **physical use after the service**
/// does (still away with a later departure, or a completed run of at least the
/// minimum). A tank-governed service at or above the exemption level, read from
/// a trustworthy reading, is exempt (`TRM-468`).
fn trip_consumes(stamp: NaiveDateTime, service: &ServiceFacts, ctx: &ValidityContext, rules: &RuleSettings) -> bool {
    let Some(marked) = ctx.trip_marked_at else { return false };
    if marked <= stamp {
        return false;
    }
    if service.governed_by_tank
        && let TankFact::Percent(percent) = ctx.tank
        && percent >= rules.garage_trip_fuel_exempt_percent
    {
        return false;
    }
    let p = &ctx.presence;
    let still_away_after = p.away && p.last_departure_at.is_some_and(|d| d > stamp);
    let run_after = p
        .last_run
        .is_some_and(|(departed, arrived)| arrived > stamp && run_counts_as_trip(departed, arrived, rules));
    still_away_after || run_after
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(day: u32, hour: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, day).unwrap().and_hms_opt(hour, 0, 0).unwrap()
    }

    fn performed(day: u32, hour: u32) -> ServiceFacts {
        ServiceFacts {
            state: GarageServiceState::Performed,
            performed_at: Some(at(day, hour)),
            marked_at: None,
            forced_pending_at: None,
            governed_by_tank: false,
        }
    }

    fn marked(state: GarageServiceState, day: u32, hour: u32) -> ServiceFacts {
        ServiceFacts { state, performed_at: None, marked_at: Some(at(day, hour)), forced_pending_at: None, governed_by_tank: false }
    }

    fn ctx(presence: PresenceFacts, now: NaiveDateTime) -> ValidityContext {
        ValidityContext { presence, trip_marked_at: None, tank: TankFact::NoUsableReading, now }
    }

    fn at_base() -> PresenceFacts {
        PresenceFacts::default()
    }

    fn departed(day: u32, hour: u32) -> PresenceFacts {
        PresenceFacts { last_departure_at: Some(at(day, hour)), away: true, ..PresenceFacts::default() }
    }

    fn run(dep: (u32, u32), arr: (u32, u32)) -> PresenceFacts {
        PresenceFacts {
            last_departure_at: Some(at(dep.0, dep.1)),
            last_arrival_at: Some(at(arr.0, arr.1)),
            away: false,
            last_run: Some((at(dep.0, dep.1), at(arr.0, arr.1))),
        }
    }

    const R: fn() -> RuleSettings = RuleSettings::default;

    #[test]
    fn pending_is_sovereign_and_a_forcing_stamp_at_or_after_the_performance_is_decisive() {
        let pending = ServiceFacts { state: GarageServiceState::Pending, performed_at: Some(at(1, 8)), ..performed(1, 8) };
        assert_eq!(effective_state(&pending, &ctx(at_base(), at(2, 8)), &R()), (GarageServiceState::Pending, ValidityReason::Pending));
        let forced = ServiceFacts { forced_pending_at: Some(at(1, 8)), ..performed(1, 8) };
        assert_eq!(effective_state(&forced, &ctx(at_base(), at(1, 9)), &R()).1, ValidityReason::ForcedPending);
    }

    #[test]
    fn a_vehicle_standing_at_the_yard_never_has_its_preparation_expire() {
        // Performed a week ago, no departure since: still valid (TRM-449, 451).
        assert_eq!(effective_state(&performed(1, 8), &ctx(at_base(), at(8, 8)), &R()), (GarageServiceState::Performed, ValidityReason::Valid));
    }

    #[test]
    fn a_performed_service_expires_the_window_after_the_last_departure_and_not_before() {
        let s = performed(1, 8);
        let p = departed(2, 6);
        assert_eq!(effective_state(&s, &ctx(p, at(3, 17)), &R()).0, GarageServiceState::Performed, "35 h after departure");
        assert_eq!(effective_state(&s, &ctx(p, at(3, 18)), &R()), (GarageServiceState::Pending, ValidityReason::Expired), "36 h after departure");
    }

    #[test]
    fn every_departure_restarts_the_window_per_the_owners_ruling() {
        // Performed day 1; departed day 2 and back; departs again day 5: the count runs from day 5.
        let s = performed(1, 8);
        let p = PresenceFacts { last_departure_at: Some(at(5, 6)), away: true, last_arrival_at: Some(at(2, 20)), last_run: Some((at(2, 6), at(2, 20))) };
        // The day-2 run is made too short to count as a trip, to isolate the window itself.
        let short = PresenceFacts { last_run: Some((at(2, 6), at(2, 6))), ..p };
        assert_eq!(effective_state(&s, &ctx(short, at(6, 5)), &R()).0, GarageServiceState::Performed, "23 h after the day-5 departure");
        assert_eq!(effective_state(&s, &ctx(short, at(6, 18)), &R()).0, GarageServiceState::Pending, "36 h after the day-5 departure");
    }

    #[test]
    fn a_return_from_a_run_that_counts_as_a_trip_resets_the_service_but_a_short_charter_does_not() {
        let s = performed(1, 8);
        // Out 10:00, back 13:00 (3 h >= 1 h): a trip return.
        assert_eq!(effective_state(&s, &ctx(run((2, 10), (2, 13)), at(2, 14)), &R()), (GarageServiceState::Pending, ValidityReason::TripReturn));
        // Out 10:00, back 10:32: a brief charter does not consume the preparation (TRM-782).
        let p = PresenceFacts { last_run: Some((at(2, 10), at(2, 10) + Duration::minutes(32))), last_arrival_at: Some(at(2, 10) + Duration::minutes(32)), ..run((2, 10), (2, 11)) };
        assert_eq!(effective_state(&s, &ctx(p, at(2, 12)), &R()).0, GarageServiceState::Performed);
    }

    #[test]
    fn a_not_needed_mark_is_consumed_by_the_next_departure_and_ended_by_the_next_arrival() {
        let m = marked(GarageServiceState::NotNeeded, 1, 8);
        // Not departed since: valid indefinitely (TRM-456).
        assert_eq!(effective_state(&m, &ctx(at_base(), at(20, 8)), &R()), (GarageServiceState::NotNeeded, ValidityReason::Valid));
        // Departs after the mark: the run consumed the reference (TRM-457).
        assert_eq!(effective_state(&m, &ctx(departed(2, 6), at(2, 7)), &R()).1, ValidityReason::ConsumedByDeparture);
        // A mark made while away documents that run: valid, with no time expiry, while away (TRM-454)...
        let away_mark = marked(GarageServiceState::NotDone, 2, 9);
        assert_eq!(effective_state(&away_mark, &ctx(departed(2, 6), at(9, 9)), &R()).0, GarageServiceState::NotDone);
        // ...and returned to pending by the physical arrival (TRM-455).
        let back = PresenceFacts { last_arrival_at: Some(at(3, 20)), away: false, ..departed(2, 6) };
        assert_eq!(effective_state(&away_mark, &ctx(back, at(3, 21)), &R()).1, ValidityReason::ReturnedToBase);
    }

    #[test]
    fn the_fuelling_service_is_governed_by_the_tank_not_by_elapsed_time() {
        let fuel = ServiceFacts { governed_by_tank: true, ..performed(1, 8) };
        let away_long = departed(2, 6); // far beyond 36 h by `now`
        let mut c = ctx(away_long, at(9, 8));
        c.tank = TankFact::Percent(72.0);
        assert_eq!(effective_state(&fuel, &c, &R()), (GarageServiceState::Performed, ValidityReason::TankGoverned));
        c.tank = TankFact::Percent(45.0);
        assert_eq!(effective_state(&fuel, &c, &R()), (GarageServiceState::Pending, ValidityReason::TankLow));
        // No usable reading: the ordinary time rule decides (TRM-462).
        c.tank = TankFact::NoUsableReading;
        assert_eq!(effective_state(&fuel, &c, &R()).1, ValidityReason::Expired);
    }

    #[test]
    fn a_marked_trip_redoes_services_only_after_physical_use_and_a_full_tank_is_exempt() {
        let s = performed(1, 8);
        // Trip marked day 2, vehicle has not moved since the service: not consumed (TRM-466).
        let mut c = ctx(at_base(), at(2, 12));
        c.trip_marked_at = Some(at(2, 9));
        assert_eq!(effective_state(&s, &c, &R()).0, GarageServiceState::Performed);
        // Still away with a departure after the service: consumed (TRM-465).
        let mut c = ctx(departed(2, 10), at(2, 12));
        c.trip_marked_at = Some(at(2, 9));
        assert_eq!(effective_state(&s, &c, &R()), (GarageServiceState::Pending, ValidityReason::TripMarked));
        // The fuelling service at >= 95 % on a trustworthy reading is exempt (TRM-468).
        let fuel = ServiceFacts { governed_by_tank: true, ..s };
        c.tank = TankFact::Percent(96.0);
        assert_eq!(effective_state(&fuel, &c, &R()).0, GarageServiceState::Performed);
        c.tank = TankFact::Percent(80.0);
        assert_eq!(effective_state(&fuel, &c, &R()).1, ValidityReason::TripMarked);
    }

    #[test]
    fn a_tenant_can_move_the_window() {
        let s = performed(1, 8);
        let p = departed(2, 6);
        let short = RuleSettings { garage_validity_hours: 12, ..R() };
        assert_eq!(effective_state(&s, &ctx(p, at(2, 17)), &short).0, GarageServiceState::Performed);
        assert_eq!(effective_state(&s, &ctx(p, at(2, 18)), &short).0, GarageServiceState::Pending);
    }
}
