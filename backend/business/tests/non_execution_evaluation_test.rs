use business::domain::enums::PresenceEventKind::{Arrival, Departure};
use business::domain::enums::{PresenceEventKind, WorkOrderStatus};
use business::use_cases::non_execution_evaluation::{
    Decision, Interval, PresenceEvent, all_resolved, build_intervals, decide, did_not_appear_key,
    entered_and_left_key, manual_evidence, manual_key,
};
use chrono::{NaiveDate, NaiveDateTime};

/// Minutes after 2026-10-10 00:00.
fn at(minutes: i64) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 10, 10).unwrap().and_hms_opt(0, 0, 0).unwrap()
        + chrono::Duration::minutes(minutes)
}

fn ev(kind: PresenceEventKind, minutes: i64) -> PresenceEvent {
    PresenceEvent { kind, at: at(minutes) }
}

// The window under test: minutes 600..720 (10:00-12:00); evaluated at 800 with a 60-minute grace
// (the window plus grace ends at 780).
const START: i64 = 600;
const END: i64 = 720;
const NOW: i64 = 800;

fn run(events: &[PresenceEvent], now: i64, grace: i32) -> Decision {
    decide(events, at(START), at(END), at(now), grace)
}

#[test]
fn an_arrival_with_no_departure_is_an_open_interval() {
    let intervals = build_intervals(&[ev(Arrival, 100)]);
    assert_eq!(intervals, vec![Interval { arrived_at: Some(at(100)), departed_at: None }]);
}

#[test]
fn a_departure_with_no_arrival_is_an_interval_of_unknown_start() {
    let intervals = build_intervals(&[ev(Departure, 100)]);
    assert_eq!(intervals, vec![Interval { arrived_at: None, departed_at: Some(at(100)) }]);
}

#[test]
fn repeated_states_are_not_new_events() {
    let intervals =
        build_intervals(&[ev(Departure, 10), ev(Departure, 20), ev(Arrival, 30), ev(Arrival, 40), ev(Departure, 50), ev(Departure, 60)]);
    assert_eq!(
        intervals,
        vec![
            Interval { arrived_at: None, departed_at: Some(at(10)) },
            Interval { arrived_at: Some(at(30)), departed_at: Some(at(50)) },
        ]
    );
}

#[test]
fn a_vehicle_with_no_events_gives_nothing() {
    assert!(build_intervals(&[]).is_empty());
    assert_eq!(run(&[], NOW, 60), Decision::Nothing);
}

#[test]
fn a_closed_interval_across_the_window_is_entered_and_left_with_its_minutes() {
    let events = [ev(Departure, 100), ev(Arrival, 650), ev(Departure, 700)];
    match run(&events, NOW, 60) {
        Decision::EnteredAndLeft(e) => {
            assert_eq!(e.arrived_at, Some(at(650)));
            assert_eq!(e.departed_at, at(700));
            assert_eq!(e.dwell_minutes, Some(50));
            assert_eq!(e.minutes_in_window, 50);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn only_the_part_inside_the_window_counts_as_minutes_in_window() {
    // Arrived 30 minutes before the window, left 30 minutes into it.
    match run(&[ev(Arrival, 570), ev(Departure, 630)], NOW, 60) {
        Decision::EnteredAndLeft(e) => {
            assert_eq!(e.dwell_minutes, Some(60));
            assert_eq!(e.minutes_in_window, 30);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_departure_with_an_unknown_arrival_has_no_dwell() {
    match run(&[ev(Departure, 630)], NOW, 60) {
        Decision::EnteredAndLeft(e) => {
            assert_eq!(e.arrived_at, None);
            assert_eq!(e.dwell_minutes, None);
            assert_eq!(e.minutes_in_window, 30);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_open_interval_covering_the_window_is_still_at_the_base() {
    assert_eq!(run(&[ev(Departure, 100), ev(Arrival, 650)], NOW, 60), Decision::Nothing);
}

#[test]
fn present_since_before_the_window_gives_nothing() {
    assert_eq!(run(&[ev(Arrival, 100)], NOW, 60), Decision::Nothing);
}

#[test]
fn a_departure_before_the_window_is_not_a_crossing() {
    // Arrived and left well before the window opened, then absent: that is an absence.
    assert_eq!(run(&[ev(Arrival, 100), ev(Departure, 500)], NOW, 60), Decision::DidNotAppear);
}

#[test]
fn out_since_before_the_window_and_never_back_did_not_appear() {
    assert_eq!(run(&[ev(Departure, 100)], NOW, 60), Decision::DidNotAppear);
}

#[test]
fn the_grace_must_pass_before_did_not_appear() {
    let events = [ev(Departure, 100)];
    assert_eq!(run(&events, END + 60, 60), Decision::Nothing, "exactly at the end of the grace");
    assert_eq!(run(&events, END + 61, 60), Decision::DidNotAppear);
    assert_eq!(run(&events, END + 1, 0), Decision::DidNotAppear, "no grace");
    assert_eq!(run(&events, END, 0), Decision::Nothing, "the window itself is not over yet");
}

#[test]
fn no_known_state_before_the_window_never_claims_an_absence() {
    // The first event ever is after the window opened: the tracker says nothing about the start.
    assert_eq!(run(&[ev(Arrival, 800)], 900, 60), Decision::Nothing);
}

#[test]
fn an_arrival_inside_the_window_withholds_the_absence() {
    assert_eq!(run(&[ev(Departure, 100), ev(Arrival, 650)], NOW, 60), Decision::Nothing);
}

#[test]
fn an_arrival_only_after_the_window_withholds_the_absence() {
    // Still there: not a departure yet, and the absence is not claimed.
    assert_eq!(run(&[ev(Departure, 100), ev(Arrival, 790)], 900, 60), Decision::Nothing);
}

#[test]
fn a_late_visit_that_left_is_an_entered_and_left_with_no_minutes_in_the_window() {
    // The window ended at 700; the bus came at 790 and left at 850 with the order unresolved (TRM-261).
    let Decision::EnteredAndLeft(evidence) = run(&[ev(Departure, 100), ev(Arrival, 790), ev(Departure, 850)], 900, 60) else {
        panic!("a late visit that left must be recorded");
    };
    assert_eq!(evidence.dwell_minutes, Some(60));
    assert_eq!(evidence.minutes_in_window, 0);
}

#[test]
fn a_visit_that_ended_before_the_window_opened_says_nothing_about_it() {
    // Window opens at 600: a visit 200..300 is history, the absence is still the absence.
    assert_eq!(run(&[ev(Departure, 100), ev(Arrival, 200), ev(Departure, 300)], NOW, 60), Decision::DidNotAppear);
}

#[test]
fn entered_and_left_takes_precedence_over_did_not_appear() {
    // Out at the start, then a visit across the window: both would apply on a naive reading.
    let events = [ev(Departure, 100), ev(Arrival, 610), ev(Departure, 640)];
    assert!(matches!(run(&events, NOW, 60), Decision::EnteredAndLeft(_)));
}

#[test]
fn oscillation_outside_the_window_is_not_a_new_departure() {
    // Blips well before the window, a second departure repeated: still just an absence.
    let events = [ev(Departure, 100), ev(Arrival, 200), ev(Departure, 210), ev(Departure, 215)];
    assert_eq!(run(&events, NOW, 60), Decision::DidNotAppear);
    // And with only a departure repeated inside the window: not a visit.
    let repeated = [ev(Departure, 100), ev(Departure, 650)];
    assert_eq!(run(&repeated, NOW, 60), Decision::DidNotAppear);
}

#[test]
fn the_first_event_being_old_still_anchors_the_state() {
    // The anchor (last event before the window) is days old: out since then.
    let days = 3 * 24 * 60;
    let events = [ev(Departure, START - days)];
    assert_eq!(run(&events, NOW, 60), Decision::DidNotAppear);
    // Present since then (an old arrival), leaving in the window: it entered long ago and left.
    match run(&[ev(Arrival, START - days), ev(Departure, 650)], NOW, 60) {
        Decision::EnteredAndLeft(e) => {
            assert_eq!(e.dwell_minutes, Some(days as i32 + 50));
            assert_eq!(e.minutes_in_window, 50);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_plan_concludes_only_when_it_has_orders_and_all_are_resolved() {
    use WorkOrderStatus::*;
    assert!(!all_resolved(&[]));
    assert!(all_resolved(&[Concluded, Cancelled]));
    assert!(!all_resolved(&[Concluded, Open]));
    assert!(!all_resolved(&[PartiallyResolved]));
    assert!(!all_resolved(&[AwaitingParts, Concluded]));
}

#[test]
fn the_keys_are_deterministic_and_distinct_per_kind() {
    let a = did_not_appear_key(7, 9, at(START));
    assert_eq!(a, did_not_appear_key(7, 9, at(START)));
    assert_ne!(a, did_not_appear_key(7, 10, at(START)));
    let Decision::EnteredAndLeft(e) = run(&[ev(Arrival, 650), ev(Departure, 700)], NOW, 60) else {
        panic!("expected a visit");
    };
    let visit = entered_and_left_key(7, 9, &e);
    assert_ne!(visit, a);
    assert_eq!(visit, format!("7:9:{}", at(650).and_utc().timestamp()));
    assert_eq!(manual_key(7, 9, &e), format!("manual:{visit}"));
    // With no known arrival the departure anchors the key.
    let Decision::EnteredAndLeft(unknown) = run(&[ev(Departure, 630)], NOW, 60) else {
        panic!("expected a visit");
    };
    assert_eq!(entered_and_left_key(7, 9, &unknown), format!("7:9:{}", at(630).and_utc().timestamp()));
}

#[test]
fn manual_evidence_needs_a_crossing_visit_and_enough_dwell_from_the_arrival() {
    let visit = [ev(Departure, 100), ev(Arrival, 650), ev(Departure, 665)];
    let e = manual_evidence(&visit, at(START), at(END), 10).expect("15 minutes >= 10");
    assert_eq!((e.dwell_minutes, e.minutes_in_window), (Some(15), 15));
    assert!(manual_evidence(&visit, at(START), at(END), 16).is_none(), "below the tenant's minimum");
    assert!(manual_evidence(&[ev(Departure, 630)], at(START), at(END), 1).is_none(), "an unknown arrival is not evidence");
    assert!(manual_evidence(&[ev(Departure, 100), ev(Arrival, 650)], at(START), at(END), 1).is_none(), "still inside");
    assert!(manual_evidence(&[ev(Arrival, 100), ev(Departure, 500)], at(START), at(END), 1).is_none(), "outside the window");
    assert!(manual_evidence(&[], at(START), at(END), 1).is_none());
}

#[test]
fn the_dwell_is_measured_from_the_arrival_not_from_when_the_plan_was_made() {
    // Arrived 20 hours before the window; only 5 minutes of it inside. The dwell is the whole stay.
    let events = [ev(Arrival, START - 20 * 60), ev(Departure, START + 5)];
    let e = manual_evidence(&events, at(START), at(END), 10).expect("long stay");
    assert_eq!(e.dwell_minutes, Some(20 * 60 + 5));
    assert_eq!(e.minutes_in_window, 5);
}
