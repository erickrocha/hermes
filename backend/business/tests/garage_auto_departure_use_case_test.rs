use business::domain::enums::{GarageAttendanceOrigin, GarageAttendanceStatus};
use business::domain::garage_attendance::GarageAttendance;
use business::use_cases::effective_schedule::ItemKind;
use business::use_cases::effective_schedule_use_case::Departure;
use business::use_cases::garage_auto_departure_use_case::{
    automatic_departure_justification, due_departure,
};
use chrono::NaiveDateTime;

fn at(hour: u32, minute: u32) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(
        &format!("2026-10-14 {hour:02}:{minute:02}:00"),
        "%Y-%m-%d %H:%M:%S",
    )
    .expect("valid test time")
}

fn attendance(checked_in_at: NaiveDateTime) -> GarageAttendance {
    GarageAttendance {
        id: Some(1),
        uuid: Some("attendance-1".into()),
        tenant_id: Some(7),
        vehicle_id: 11,
        attendance_date: checked_in_at.date(),
        checked_in_at,
        status: GarageAttendanceStatus::Open,
        manual_priority: Some(2),
        released_at: None,
        automatic_departure_justification: None,
        origin: GarageAttendanceOrigin::Manual,
        active_marker: Some(1),
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn departure(at: NaiveDateTime, kind: ItemKind, name: &str) -> Departure {
    Departure {
        at,
        is_trip: matches!(kind, ItemKind::OneOffTrip | ItemKind::ExtraTrip),
        kind,
        trip_id: None,
        name: Some(name.to_string()),
    }
}

#[test]
fn only_due_trip_or_charter_starts_after_check_in() {
    let triage = attendance(at(8, 0));
    let departures = vec![
        departure(at(7, 30), ItemKind::OneOffTrip, "already underway"),
        departure(at(9, 0), ItemKind::Line, "recurring line"),
        departure(at(10, 0), ItemKind::ExtraTrip, "order 42"),
        departure(at(11, 0), ItemKind::ExtraLine, "charter 19"),
    ];

    assert!(due_departure(&triage, &departures, at(9, 59)).is_none());
    assert_eq!(
        due_departure(&triage, &departures, at(12, 0)).map(|item| item.name.as_deref()),
        Some(Some("order 42"))
    );
}

#[test]
fn a_departure_window_open_at_check_in_does_not_close_the_triage() {
    let triage = attendance(at(8, 0));
    let departures = vec![
        departure(at(7, 30), ItemKind::OneOffTrip, "started earlier"),
        departure(at(8, 0), ItemKind::ExtraLine, "starts at check in"),
        departure(at(9, 0), ItemKind::Line, "recurring line"),
    ];

    assert!(due_departure(&triage, &departures, at(12, 0)).is_none());
}

#[test]
fn justification_names_the_departure_type_time_and_unperformed_services() {
    let scheduled = departure(at(10, 15), ItemKind::ExtraLine, "charter 19");
    let reason = automatic_departure_justification(
        &scheduled,
        &["External wash".into(), "Interior cleaning".into()],
    );

    assert!(reason.contains("fretamento 'charter 19'"));
    assert!(reason.contains("2026-10-14 10:15"));
    assert!(reason.contains("External wash, Interior cleaning"));
}
