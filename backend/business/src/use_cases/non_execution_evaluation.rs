//! `EPIC-MT-04` (`C-026`): the pure part of the non-execution evaluation -- presence as
//! intervals and the decision for one planned window. No database, no clock: the caller
//! passes the events and "now", so every edge case is a plain unit test.

use crate::domain::enums::{PresenceEventKind, WorkOrderStatus};
use chrono::{Duration, NaiveDateTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresenceEvent {
    pub kind: PresenceEventKind,
    pub at: NaiveDateTime,
}

/// `[arrived_at, departed_at)`. No arrival: the start is unknown. No departure: still there (open).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub arrived_at: Option<NaiveDateTime>,
    pub departed_at: Option<NaiveDateTime>,
}

/// What a closed interval says about a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Evidence {
    pub arrived_at: Option<NaiveDateTime>,
    pub departed_at: NaiveDateTime,
    /// Departure minus arrival, whole minutes; `None` when the arrival is unknown.
    pub dwell_minutes: Option<i32>,
    pub minutes_in_window: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Nothing,
    EnteredAndLeft(Evidence),
    DidNotAppear,
}

/// `SR-MT-04-02`: the events (oldest first) as intervals. A second arrival while inside and a
/// departure while already outside are repeats of a state, not new events, and are ignored; only
/// a departure that is the very first event is an interval with an unknown start.
pub fn build_intervals(events: &[PresenceEvent]) -> Vec<Interval> {
    let mut intervals = Vec::new();
    let mut open: Option<NaiveDateTime> = None;
    for (index, event) in events.iter().enumerate() {
        match event.kind {
            PresenceEventKind::Arrival => {
                open.get_or_insert(event.at);
            }
            PresenceEventKind::Departure => match open.take() {
                Some(arrived) => intervals.push(Interval {
                    arrived_at: Some(arrived),
                    departed_at: Some(event.at),
                }),
                None if index == 0 => intervals.push(Interval {
                    arrived_at: None,
                    departed_at: Some(event.at),
                }),
                None => {}
            },
        }
    }
    if let Some(arrived) = open {
        intervals.push(Interval {
            arrived_at: Some(arrived),
            departed_at: None,
        });
    }
    intervals
}

fn crosses(interval: &Interval, start: NaiveDateTime, end: NaiveDateTime) -> bool {
    interval.arrived_at.is_none_or(|arrived| arrived < end)
        && interval.departed_at.is_none_or(|departed| departed > start)
}

fn evidence_of(interval: &Interval, departed: NaiveDateTime, start: NaiveDateTime, end: NaiveDateTime) -> Evidence {
    let inside_from = interval.arrived_at.map_or(start, |arrived| arrived.max(start));
    Evidence {
        arrived_at: interval.arrived_at,
        departed_at: departed,
        dwell_minutes: interval.arrived_at.map(|arrived| minutes(departed - arrived)),
        minutes_in_window: minutes(departed.min(end) - inside_from).max(0),
    }
}

fn minutes(span: Duration) -> i32 {
    i32::try_from(span.num_minutes()).unwrap_or(i32::MAX)
}

/// The first closed interval that left after the window opened: a visit across `[start, end]` or a
/// late one, after it (`TRM-261`: leaving the base with the plan's orders unresolved needs no crossing).
/// A visit that ended before the window opened says nothing about it. `minutes_in_window` is 0 for a late visit.
pub fn entered_and_left(intervals: &[Interval], start: NaiveDateTime, end: NaiveDateTime) -> Option<Evidence> {
    intervals.iter().find_map(|interval| {
        let departed = interval.departed_at?;
        (departed > start).then(|| evidence_of(interval, departed, start, end))
    })
}

/// `SR-MT-04-03/04/05`. `events` are oldest first and must include the vehicle's last event
/// before `start` (however old): that event is the state when the window opened.
pub fn decide(
    events: &[PresenceEvent],
    start: NaiveDateTime,
    end: NaiveDateTime,
    now: NaiveDateTime,
    grace_minutes: i32,
) -> Decision {
    let intervals = build_intervals(events);
    if let Some(evidence) = entered_and_left(&intervals, start, end) {
        return Decision::EnteredAndLeft(evidence);
    }
    if now <= end + Duration::minutes(i64::from(grace_minutes)) {
        return Decision::Nothing;
    }
    // Only a known "out" counts: no event before the window means the tracker says nothing.
    let known_out = events
        .iter()
        .rfind(|event| event.at < start)
        .is_some_and(|event| event.kind == PresenceEventKind::Departure);
    // Any presence from the window's start on, late arrivals included, withholds the claim.
    let seen_since = intervals
        .iter()
        .any(|interval| interval.departed_at.is_none_or(|departed| departed > start));
    if known_out && !seen_since {
        Decision::DidNotAppear
    } else {
        Decision::Nothing
    }
}

/// `SR-MT-04-10`: the closed interval crossing the window whose dwell, **measured from the
/// arrival**, reaches `min_dwell_minutes`. An unknown arrival is not evidence.
pub fn manual_evidence(
    events: &[PresenceEvent],
    start: NaiveDateTime,
    end: NaiveDateTime,
    min_dwell_minutes: i32,
) -> Option<Evidence> {
    build_intervals(events).iter().find_map(|interval| {
        let departed = interval.departed_at?;
        let evidence = evidence_of(interval, departed, start, end);
        (crosses(interval, start, end) && evidence.dwell_minutes? >= min_dwell_minutes).then_some(evidence)
    })
}

/// `SR-MT-04-01`: a plan concludes only when it has orders and every one is resolved.
pub fn all_resolved(statuses: &[WorkOrderStatus]) -> bool {
    !statuses.is_empty() && statuses.iter().all(is_resolved)
}

pub fn is_resolved(status: &WorkOrderStatus) -> bool {
    matches!(status, WorkOrderStatus::Concluded | WorkOrderStatus::Cancelled)
}

/// `SR-MT-04-06`: the deterministic key of an automatic occurrence.
pub fn did_not_appear_key(plan_id: i64, order_id: i64, window_start: NaiveDateTime) -> String {
    format!("{plan_id}:{order_id}:{}", window_start.and_utc().timestamp())
}

/// The arrival, or the departure when the arrival is unknown.
pub fn entered_and_left_key(plan_id: i64, order_id: i64, evidence: &Evidence) -> String {
    let anchor = evidence.arrived_at.unwrap_or(evidence.departed_at);
    format!("{plan_id}:{order_id}:{}", anchor.and_utc().timestamp())
}

pub fn manual_key(plan_id: i64, order_id: i64, evidence: &Evidence) -> String {
    format!("manual:{}", entered_and_left_key(plan_id, order_id, evidence))
}
