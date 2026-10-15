use crate::domain::enums::PresenceEventKind;
use crate::domain::garage_cycle::GarageCycleState;
use crate::domain::tenant_rule_setting::RuleSettings;
use chrono::NaiveDateTime;

#[derive(Debug, Clone, Copy)]
pub struct GarageCycleSample {
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub reported_at: NaiveDateTime,
    pub at_base: bool,
    pub transition: Option<PresenceEventKind>,
    pub odometer_meters: Option<f64>,
    pub ignition_on: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleSampleResult {
    Updated,
    Ignored,
    OdometerGlitch,
}

pub fn observe(
    state: &mut GarageCycleState,
    sample: GarageCycleSample,
    rules: &RuleSettings,
) -> CycleSampleResult {
    let odometer = sample
        .odometer_meters
        .filter(|value| value.is_finite() && *value >= 0.0);
    if let Some(last_at) = state.last_observed_at
        && sample.reported_at <= last_at
    {
        return CycleSampleResult::Ignored;
    }

    let previous_odometer = state.last_observed_odometer_meters;
    let distance_delta = match (previous_odometer, odometer) {
        (Some(previous), Some(current)) if current >= previous => current - previous,
        _ => 0.0,
    };
    let max_jump_meters = rules.garage_odometer_max_single_jump_km * 1000.0;
    if distance_delta > max_jump_meters {
        return CycleSampleResult::OdometerGlitch;
    }

    let fallback_meters = rules.garage_cycle_fallback_distance_km * 1000.0;
    let may_accumulate = sample.ignition_on != Some(false);

    if sample.transition == Some(PresenceEventKind::Departure) && !state.cycle_is_open() {
        state.open_cycle_started_at = Some(sample.reported_at);
        state.open_cycle_start_odometer_meters = odometer;
        state.open_cycle_distance_meters = 0.0;
    } else if state.cycle_is_open() {
        if may_accumulate {
            state.open_cycle_distance_meters += distance_delta;
        }
        if sample.at_base {
            close_cycle(state, odometer, sample.reported_at);
        }
    } else if sample.at_base {
        match (state.reference_odometer_meters, odometer) {
            (None, Some(current)) => state.reference_odometer_meters = Some(current),
            (Some(reference), Some(current)) if current >= reference => {
                let cycle_distance = current - reference;
                if cycle_distance >= fallback_meters {
                    state.last_completed_cycle_distance_meters = Some(cycle_distance);
                    state.last_completed_cycle_at = Some(sample.reported_at);
                    state.reference_odometer_meters = Some(current);
                }
            }
            _ => {}
        }
    } else if let (Some(reference), Some(current)) = (state.reference_odometer_meters, odometer)
        && current >= reference
        && current - reference >= fallback_meters
    {
        state.open_cycle_started_at = Some(sample.reported_at);
        state.open_cycle_start_odometer_meters = Some(reference);
        state.open_cycle_distance_meters = if may_accumulate {
            current - reference
        } else {
            0.0
        };
    } else if state.reference_odometer_meters.is_none()
        && let Some(current) = odometer
    {
        state.reference_odometer_meters = Some(current);
    }

    if let Some(current) = odometer {
        state.last_observed_odometer_meters = Some(current);
    }
    state.last_observed_at = Some(sample.reported_at);
    state.tenant_id = sample.tenant_id;
    state.vehicle_id = sample.vehicle_id;
    CycleSampleResult::Updated
}

fn close_cycle(
    state: &mut GarageCycleState,
    return_odometer_meters: Option<f64>,
    returned_at: NaiveDateTime,
) {
    state.last_completed_cycle_distance_meters = Some(state.open_cycle_distance_meters);
    state.last_completed_cycle_at = Some(returned_at);
    state.open_cycle_started_at = None;
    state.open_cycle_start_odometer_meters = None;
    state.open_cycle_distance_meters = 0.0;
    if let Some(odometer) = return_odometer_meters {
        state.reference_odometer_meters = Some(odometer);
    }
}
