use business::domain::enums::PresenceEventKind;
use business::domain::tenant_rule_setting::RuleSettings;
use business::gateway::tracking_provider::ProviderPosition;
use business::use_cases::garage_geofence::{
    distance_meters, is_within_base, transition_for_position,
};
use chrono::{DateTime, Duration, Utc};

fn position(latitude: f64, longitude: f64, age_seconds: i64, valid: bool) -> ProviderPosition {
    let now = DateTime::parse_from_rfc3339("2026-10-13T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    ProviderPosition {
        device_id: 7,
        latitude,
        longitude,
        valid,
        recorded_at: Some(now - Duration::seconds(age_seconds)),
        ignition: None,
        odometer_meters: None,
    }
}

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-10-13T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

#[test]
fn classifies_positions_inside_and_outside_the_configured_radius() {
    let inside_latitude = 0.0005;
    let outside_latitude = 0.001;

    assert!(distance_meters(0.0, 0.0, inside_latitude, 0.0) < 75.0);
    assert!(distance_meters(0.0, 0.0, outside_latitude, 0.0) > 75.0);
    assert!(is_within_base(inside_latitude, 0.0, 0.0, 0.0, 75.0));
    assert!(!is_within_base(outside_latitude, 0.0, 0.0, 0.0, 75.0));
}

#[test]
fn includes_positions_on_the_geofence_boundary() {
    let boundary_latitude = 75.0_f64.to_degrees() / 6_371_000.0;

    assert!(is_within_base(boundary_latitude, 0.0, 0.0, 0.0, 75.0));
}

#[test]
fn records_only_fresh_transitions_across_the_base_boundary() {
    let settings = RuleSettings {
        garage_base_latitude: Some(0.0),
        garage_base_longitude: Some(0.0),
        ..RuleSettings::default()
    };
    let inside = position(0.0005, 0.0, 30, true);
    let outside = position(0.001, 0.0, 30, true);

    assert_eq!(
        transition_for_position(&inside, &settings, true, now()),
        Some(PresenceEventKind::Arrival)
    );
    assert_eq!(
        transition_for_position(&outside, &settings, false, now()),
        Some(PresenceEventKind::Departure)
    );
    assert_eq!(
        transition_for_position(&inside, &settings, false, now()),
        None
    );
    assert_eq!(
        transition_for_position(&outside, &settings, true, now()),
        None
    );
}

#[test]
fn ignores_missing_base_invalid_positions_and_stale_positions() {
    let settings = RuleSettings::default();
    let inside = position(0.0, 0.0, 30, true);
    let invalid = position(0.0, 0.0, 30, false);
    let stale = position(0.0, 0.0, 301, true);
    let configured = RuleSettings {
        garage_base_latitude: Some(0.0),
        garage_base_longitude: Some(0.0),
        ..RuleSettings::default()
    };

    assert_eq!(
        transition_for_position(&inside, &settings, false, now()),
        None
    );
    assert_eq!(
        transition_for_position(&invalid, &configured, false, now()),
        None
    );
    assert_eq!(
        transition_for_position(&stale, &configured, false, now()),
        None
    );
}
