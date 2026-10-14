use business::domain::enums::PresenceEventKind;
use business::domain::garage_cycle::GarageCycleState;
use business::domain::tenant_rule_setting::RuleSettings;
use business::use_cases::garage_cycle::{CycleSampleResult, GarageCycleSample, observe};
use chrono::NaiveDateTime;

fn at(seconds: u32) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(
        &format!("2026-10-13 12:00:{seconds:02}"),
        "%Y-%m-%d %H:%M:%S",
    )
    .unwrap()
}

fn sample(
    at: NaiveDateTime,
    at_base: bool,
    transition: Option<PresenceEventKind>,
    odometer_meters: f64,
    ignition_on: Option<bool>,
) -> GarageCycleSample {
    GarageCycleSample {
        tenant_id: Some(1),
        vehicle_id: 7,
        reported_at: at,
        at_base,
        transition,
        odometer_meters: Some(odometer_meters),
        ignition_on,
    }
}

#[test]
fn a_departure_opens_and_a_return_completes_one_cycle() {
    let rules = RuleSettings::default();
    let mut state = GarageCycleState::new(Some(1), 7);
    observe(
        &mut state,
        sample(at(0), true, None, 100_000.0, Some(false)),
        &rules,
    );
    observe(
        &mut state,
        sample(
            at(1),
            false,
            Some(PresenceEventKind::Departure),
            100_000.0,
            Some(true),
        ),
        &rules,
    );
    observe(
        &mut state,
        sample(at(2), false, None, 101_000.0, Some(true)),
        &rules,
    );
    observe(
        &mut state,
        sample(at(3), false, None, 101_500.0, Some(false)),
        &rules,
    );
    observe(
        &mut state,
        sample(at(4), false, None, 102_000.0, Some(true)),
        &rules,
    );
    observe(
        &mut state,
        sample(
            at(5),
            true,
            Some(PresenceEventKind::Arrival),
            102_100.0,
            Some(true),
        ),
        &rules,
    );

    assert!(!state.cycle_is_open());
    assert_eq!(state.last_completed_cycle_distance_meters, Some(1_600.0));
    assert_eq!(state.reference_odometer_meters, Some(102_100.0));
}

#[test]
fn a_fallback_cycle_opens_once_after_the_configured_distance() {
    let rules = RuleSettings::default();
    let mut state = GarageCycleState::new(Some(1), 7);
    observe(
        &mut state,
        sample(at(0), true, None, 10_000.0, Some(false)),
        &rules,
    );
    observe(
        &mut state,
        sample(at(1), false, None, 11_500.0, Some(true)),
        &rules,
    );
    assert!(!state.cycle_is_open());
    observe(
        &mut state,
        sample(at(2), false, None, 12_000.0, Some(true)),
        &rules,
    );
    assert!(state.cycle_is_open());
    observe(
        &mut state,
        sample(at(3), false, None, 12_500.0, Some(true)),
        &rules,
    );
    assert_eq!(state.open_cycle_distance_meters, 2_500.0);
}

#[test]
fn a_run_detected_while_at_base_completes_and_reanchors_the_fallback_cycle() {
    let rules = RuleSettings::default();
    let mut state = GarageCycleState::new(Some(1), 7);
    observe(
        &mut state,
        sample(at(0), true, None, 50_000.0, Some(false)),
        &rules,
    );
    observe(
        &mut state,
        sample(at(1), true, None, 52_100.0, Some(false)),
        &rules,
    );

    assert_eq!(state.last_completed_cycle_distance_meters, Some(2_100.0));
    assert_eq!(state.reference_odometer_meters, Some(52_100.0));
}

#[test]
fn an_unreasonable_single_reading_jump_is_ignored() {
    let rules = RuleSettings::default();
    let mut state = GarageCycleState::new(Some(1), 7);
    observe(
        &mut state,
        sample(at(0), true, None, 10_000.0, Some(false)),
        &rules,
    );

    let result = observe(
        &mut state,
        sample(at(1), false, None, 400_001.0, Some(true)),
        &rules,
    );

    assert_eq!(result, CycleSampleResult::OdometerGlitch);
    assert_eq!(state.last_observed_odometer_meters, Some(10_000.0));
    assert!(!state.cycle_is_open());
}
