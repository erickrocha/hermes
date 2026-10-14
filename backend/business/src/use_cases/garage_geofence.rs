use crate::domain::enums::PresenceEventKind;
use crate::domain::tenant_rule_setting::RuleSettings;
use crate::gateway::tracking_provider::ProviderPosition;
use crate::use_cases::vehicle_tracking_use_case::POSITION_FRESHNESS_SECONDS;
use chrono::{DateTime, Utc};

const EARTH_RADIUS_METERS: f64 = 6_371_000.0;

pub fn distance_meters(
    latitude_a: f64,
    longitude_a: f64,
    latitude_b: f64,
    longitude_b: f64,
) -> f64 {
    let latitude_delta = (latitude_b - latitude_a).to_radians();
    let longitude_delta = (longitude_b - longitude_a).to_radians();
    let latitude_a = latitude_a.to_radians();
    let latitude_b = latitude_b.to_radians();

    let haversine = (latitude_delta / 2.0).sin().powi(2)
        + latitude_a.cos() * latitude_b.cos() * (longitude_delta / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_METERS * haversine.clamp(0.0, 1.0).sqrt().asin()
}

pub fn is_within_base(
    latitude: f64,
    longitude: f64,
    base_latitude: f64,
    base_longitude: f64,
    radius_meters: f64,
) -> bool {
    distance_meters(latitude, longitude, base_latitude, base_longitude) <= radius_meters
}

pub fn transition_for_position(
    position: &ProviderPosition,
    settings: &RuleSettings,
    away: bool,
    now: DateTime<Utc>,
) -> Option<PresenceEventKind> {
    if !position.valid {
        return None;
    }
    let reported_at = position.recorded_at?;
    let age = now.signed_duration_since(reported_at).num_seconds();
    if !(0..=POSITION_FRESHNESS_SECONDS).contains(&age) {
        return None;
    }

    let (base_latitude, base_longitude) = (
        settings.garage_base_latitude?,
        settings.garage_base_longitude?,
    );
    let at_base = is_within_base(
        position.latitude,
        position.longitude,
        base_latitude,
        base_longitude,
        settings.garage_base_radius_meters,
    );

    match (away, at_base) {
        (true, true) => Some(PresenceEventKind::Arrival),
        (false, false) => Some(PresenceEventKind::Departure),
        _ => None,
    }
}
