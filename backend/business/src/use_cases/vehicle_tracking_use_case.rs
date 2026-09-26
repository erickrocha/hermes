//! Vehicle tracking ingestion — the rules (EPIC-FT-01, `HRMS-900`…`HRMS-905`).
//!
//! This module decides *what a reading means*. The provider's wire format is
//! someone else's problem (`gateway::tracking_provider`), and that split is
//! `HRMS-902`: change the provider and this file should not move.
//!
//! ## Nothing here is persisted, deliberately
//!
//! `HRMS-905` requires the system to "state what consumes ingested tracking
//! data **before** that data is persisted". The consumers under discussion —
//! the garage-service queue and the fuel cycle — are `operacao-trm`'s decision
//! `D-01`, which is open and is **not hermes' to take**. So this slice builds
//! the ingestion and stops there: there is no tracking table, no entity and no
//! migration in it. That is the requirement being met, not a gap in it.
//!
//! The shape of what a store would hold is nonetheless settled by
//! `VehicleTrackingStatus`, which carries value, provenance, quality and an
//! as-of instant — the four properties `PD-025`'s note argues `D-01` needs.
//!
//! ## The degradation rules are inherited, not invented
//!
//! `D-15` records them from a production integration, and `operacao-trm` pays
//! for two of them in its own notes after real incidents. They are reproduced
//! here with the reasoning attached, because a rule whose cost is forgotten is
//! a rule someone later "simplifies".

use crate::domain::business_error::BusinessError;
use crate::domain::vehicle_tracking::{
    GeoPoint, IgnitionState, TrackingSource, VehicleTrackingStatus,
};
use crate::gateway::tracking_provider::{ProviderPosition, TrackingProvider};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

/// A position older than this is not evidence of where the vehicle is now.
/// Five minutes is the production value recorded in `D-15`.
pub const POSITION_FRESHNESS_SECONDS: i64 = 300;

/// How far back the ignition-event report is consulted when the live position
/// cannot be trusted. A vehicle parked overnight has no recent position but a
/// perfectly good `ignitionOff` from yesterday evening.
pub const EVENT_LOOKBACK_HOURS: i64 = 24;

/// The component name stamped on anything this ingestion writes, so a row is
/// attributable to the integration rather than to `system` (see
/// `entity::audit::run_for_tenant`).
pub const TRACKING_ACTOR: &str = "fleet-telemetry";

pub struct VehicleTrackingUseCase<P: TrackingProvider> {
    provider: P,
}

impl<P: TrackingProvider> VehicleTrackingUseCase<P> {
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    /// Ingests tracking state for one tenant, under that tenant's **named**
    /// scope (`HRMS-903`).
    ///
    /// The scope matters even though this slice persists nothing. `D-05` made
    /// the out-of-request default `Denied`, and the only escape hatch before
    /// `run_for_tenant` existed was `run_as_platform`, which grants the entire
    /// platform. A scheduled poller reaching for that would be precisely the
    /// accident `U-016` was raised about. Establishing the correct scope now
    /// means the consumer `D-01` eventually names is written *inside* a
    /// correctly scoped context rather than retrofitted into one.
    pub async fn ingest_for_tenant(
        &self,
        tenant_id: i64,
        device_ids: &[i64],
    ) -> Result<Vec<VehicleTrackingStatus>, BusinessError> {
        entity::audit::run_for_tenant(
            tenant_id,
            TRACKING_ACTOR,
            self.current_status(device_ids, Utc::now()),
        )
        .await
    }

    /// The rules, with `now` injected so freshness is testable without
    /// sleeping. Callers outside tests want `ingest_for_tenant`.
    pub async fn current_status(
        &self,
        device_ids: &[i64],
        now: DateTime<Utc>,
    ) -> Result<Vec<VehicleTrackingStatus>, BusinessError> {
        if device_ids.is_empty() {
            return Ok(Vec::new());
        }

        // One call for the whole fleet: Traccar's `/positions` is not
        // per-device, and asking per device would multiply requests by the
        // fleet size. A failure here *does* fail the batch — with no position
        // feed at all there is nothing to degrade from.
        let positions = self.provider.last_positions().await?;
        let by_device: HashMap<i64, &ProviderPosition> =
            positions.iter().map(|p| (p.device_id, p)).collect();

        let mut statuses: Vec<VehicleTrackingStatus> = Vec::with_capacity(device_ids.len());
        let mut needs_fallback: Vec<i64> = Vec::new();

        for &device_id in device_ids {
            match by_device.get(&device_id) {
                Some(position) if is_trustworthy(position, now) => {
                    statuses.push(VehicleTrackingStatus {
                        device_id,
                        // `is_trustworthy` has established that ignition is present.
                        ignition: match position.ignition {
                            Some(true) => IgnitionState::On,
                            _ => IgnitionState::Off,
                        },
                        source: TrackingSource::LastPosition,
                        reported_at: position.recorded_at,
                        trustworthy: true,
                        position: Some(point(position)),
                        position_reported_at: position.recorded_at,
                        position_live: true,
                        odometer_meters: position.odometer_meters,
                    });
                }
                _ => {
                    needs_fallback.push(device_id);
                    // Provisional answer, replaced below if an event is found.
                    // The last known coordinate is attached even though the
                    // reading is stale: a parked vehicle stops emitting but
                    // has not moved, and a garage view still needs to draw it.
                    let mut unknown = VehicleTrackingStatus::unknown(device_id);
                    if let Some(position) = by_device.get(&device_id) {
                        unknown.position = Some(point(position));
                        unknown.position_reported_at = position.recorded_at;
                        unknown.odometer_meters = position.odometer_meters;
                    }
                    statuses.push(unknown);
                }
            }
        }

        if needs_fallback.is_empty() {
            return Ok(statuses);
        }

        // A failing events call degrades only the affected devices and never
        // fails the batch: the vehicles that answered from `/positions` are
        // still perfectly good readings, and throwing them away because an
        // unrelated report timed out would be a worse outcome than saying
        // "unknown" about the rest.
        let events = self
            .provider
            .ignition_events(
                &needs_fallback,
                now - Duration::hours(EVENT_LOOKBACK_HOURS),
                now,
            )
            .await
            .unwrap_or_default();

        // Newest event per device wins.
        let mut newest: HashMap<i64, (DateTime<Utc>, bool)> = HashMap::new();
        for event in events {
            newest
                .entry(event.device_id)
                .and_modify(|held| {
                    if event.occurred_at > held.0 {
                        *held = (event.occurred_at, event.ignition);
                    }
                })
                .or_insert((event.occurred_at, event.ignition));
        }

        for status in statuses.iter_mut() {
            if let Some((occurred_at, ignition)) = newest.get(&status.device_id) {
                status.ignition = if *ignition {
                    IgnitionState::On
                } else {
                    IgnitionState::Off
                };
                status.source = TrackingSource::IgnitionEvent;
                status.reported_at = Some(*occurred_at);
                // An ignition transition IS reliable evidence about ignition —
                // it is a recorded fact, not a guess — so the reading is
                // trustworthy. What it is *not* is evidence of current
                // position, and `supports_presence_claim` refuses it on
                // `source` alone. That is the "never tagged as at base on a
                // stale reading" rule, held in one place instead of at every
                // call site that might forget it.
                status.trustworthy = true;
            }
        }

        Ok(statuses)
    }
}

fn point(position: &ProviderPosition) -> GeoPoint {
    GeoPoint {
        latitude: position.latitude,
        longitude: position.longitude,
    }
}

/// The three conditions `D-15` records, together: a position is only evidence
/// of the current state if it is fresh, the provider flagged the fix valid,
/// and it actually carries an ignition attribute.
fn is_trustworthy(position: &ProviderPosition, now: DateTime<Utc>) -> bool {
    if !position.valid || position.ignition.is_none() {
        return false;
    }
    match position.recorded_at {
        Some(recorded_at) => {
            let age = now.signed_duration_since(recorded_at).num_seconds();
            // A timestamp in the future is clock skew, not freshness. Allowing
            // it would let a mis-set device claim permanent freshness.
            (0..=POSITION_FRESHNESS_SECONDS).contains(&age)
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::tracking_provider::ProviderIgnitionEvent;
    use entity::audit::{TenantScope, tenant_scope};
    use std::sync::Mutex;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-30T18:00:00+00:00")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn ago(seconds: i64) -> Option<DateTime<Utc>> {
        Some(now() - Duration::seconds(seconds))
    }

    fn position(device_id: i64, seconds_old: i64, ignition: Option<bool>) -> ProviderPosition {
        ProviderPosition {
            device_id,
            latitude: -23.5,
            longitude: -46.6,
            valid: true,
            recorded_at: ago(seconds_old),
            ignition,
            odometer_meters: Some(1000.0),
        }
    }

    /// Records the tenant scope in force when the provider was called, so
    /// `HRMS-903` can be asserted from the outside rather than trusted.
    struct FakeProvider {
        positions: Vec<ProviderPosition>,
        events: Option<Vec<ProviderIgnitionEvent>>,
        seen_scope: Mutex<Option<TenantScope>>,
        events_asked_for: Mutex<Vec<i64>>,
    }

    impl FakeProvider {
        fn new(positions: Vec<ProviderPosition>) -> Self {
            Self {
                positions,
                events: Some(Vec::new()),
                seen_scope: Mutex::new(None),
                events_asked_for: Mutex::new(Vec::new()),
            }
        }
        fn with_events(mut self, events: Vec<ProviderIgnitionEvent>) -> Self {
            self.events = Some(events);
            self
        }
        /// The events report is down; `last_positions` still works.
        fn with_failing_events(mut self) -> Self {
            self.events = None;
            self
        }
    }

    impl TrackingProvider for FakeProvider {
        async fn last_positions(&self) -> Result<Vec<ProviderPosition>, BusinessError> {
            *self.seen_scope.lock().unwrap() = Some(tenant_scope());
            Ok(self.positions.clone())
        }

        async fn ignition_events(
            &self,
            device_ids: &[i64],
            _from: DateTime<Utc>,
            _to: DateTime<Utc>,
        ) -> Result<Vec<ProviderIgnitionEvent>, BusinessError> {
            *self.events_asked_for.lock().unwrap() = device_ids.to_vec();
            self.events
                .clone()
                .ok_or_else(|| BusinessError::new("events report unavailable".to_string()))
        }
    }

    struct DeadProvider;
    impl TrackingProvider for DeadProvider {
        async fn last_positions(&self) -> Result<Vec<ProviderPosition>, BusinessError> {
            Err(BusinessError::new("provider unreachable".to_string()))
        }
        async fn ignition_events(
            &self,
            _device_ids: &[i64],
            _from: DateTime<Utc>,
            _to: DateTime<Utc>,
        ) -> Result<Vec<ProviderIgnitionEvent>, BusinessError> {
            Err(BusinessError::new("provider unreachable".to_string()))
        }
    }

    fn event(device_id: i64, seconds_old: i64, ignition: bool) -> ProviderIgnitionEvent {
        ProviderIgnitionEvent {
            device_id,
            ignition,
            occurred_at: now() - Duration::seconds(seconds_old),
        }
    }

    #[tokio::test]
    async fn a_fresh_valid_position_with_ignition_is_the_preferred_answer() {
        let use_case =
            VehicleTrackingUseCase::new(FakeProvider::new(vec![position(1, 60, Some(true))]));
        let statuses = use_case
            .current_status(&[1], now())
            .await
            .expect("positions answered");

        assert_eq!(statuses.len(), 1);
        let s = &statuses[0];
        assert_eq!(s.ignition, IgnitionState::On);
        assert_eq!(s.source, TrackingSource::LastPosition);
        assert!(s.trustworthy);
        assert!(
            s.supports_presence_claim(),
            "a fresh fix is the only thing that may place a vehicle"
        );
        assert_eq!(s.position.unwrap().latitude, -23.5);
        assert_eq!(s.odometer_meters, Some(1000.0));
    }

    #[tokio::test]
    async fn a_last_known_coordinate_keeps_its_own_time_and_is_never_live() {
        // DEF-FO-04: the coordinate's time survives a stale reading.
        // DEF-FO-05: a newer ignition event makes ignition trustworthy, not the
        // two-hour-old coordinate.
        let provider = FakeProvider::new(vec![position(1, 7200, Some(true)), position(2, 7200, Some(true))])
            .with_events(vec![event(1, 3600, false)]);
        let statuses = VehicleTrackingUseCase::new(provider)
            .current_status(&[1, 2], now())
            .await
            .unwrap();

        for s in &statuses {
            assert_eq!(s.position_reported_at, ago(7200), "device {}", s.device_id);
            assert!(!s.position_live, "device {}", s.device_id);
        }
        assert!(statuses[0].trustworthy && statuses[0].reported_at == ago(3600));
        assert!(!statuses[1].trustworthy);

        let fresh = VehicleTrackingUseCase::new(FakeProvider::new(vec![position(3, 10, Some(false))]))
            .current_status(&[3], now())
            .await
            .unwrap();
        assert!(fresh[0].position_live && fresh[0].position_reported_at == ago(10));
    }

    #[tokio::test]
    async fn a_stale_position_falls_back_to_the_event_report() {
        let provider = FakeProvider::new(vec![position(
            1,
            POSITION_FRESHNESS_SECONDS + 1,
            Some(true),
        )])
        .with_events(vec![event(1, 3600, false)]);
        let statuses = VehicleTrackingUseCase::new(provider)
            .current_status(&[1], now())
            .await
            .unwrap();

        let s = &statuses[0];
        assert_eq!(
            s.ignition,
            IgnitionState::Off,
            "the event, not the stale position, decides ignition"
        );
        assert_eq!(s.source, TrackingSource::IgnitionEvent);
        assert!(
            s.trustworthy,
            "a recorded transition is a fact, not a guess"
        );
        assert!(
            !s.supports_presence_claim(),
            "THE rule: a stale reading must never leave a vehicle tagged as being at base"
        );
        assert!(
            s.position.is_some(),
            "the last known coordinate is still attached -- a parked vehicle has not moved"
        );
    }

    #[tokio::test]
    async fn an_invalid_fix_and_a_missing_ignition_both_force_the_fallback() {
        let mut invalid = position(1, 10, Some(true));
        invalid.valid = false;
        let provider = FakeProvider::new(vec![invalid, position(2, 10, None)]);
        let use_case = VehicleTrackingUseCase::new(provider);
        let statuses = use_case.current_status(&[1, 2], now()).await.unwrap();

        assert!(
            statuses
                .iter()
                .all(|s| s.source != TrackingSource::LastPosition)
        );
        assert_eq!(
            use_case
                .provider
                .events_asked_for
                .lock()
                .unwrap()
                .as_slice(),
            &[1, 2]
        );
    }

    #[tokio::test]
    async fn a_position_dated_in_the_future_is_clock_skew_not_freshness() {
        let mut skewed = position(1, 0, Some(true));
        skewed.recorded_at = Some(now() + Duration::hours(2));
        let statuses = VehicleTrackingUseCase::new(FakeProvider::new(vec![skewed]))
            .current_status(&[1], now())
            .await
            .unwrap();
        assert_ne!(statuses[0].source, TrackingSource::LastPosition);
    }

    #[tokio::test]
    async fn no_position_and_no_event_is_unknown_and_flagged_unreliable() {
        let statuses = VehicleTrackingUseCase::new(FakeProvider::new(Vec::new()))
            .current_status(&[7], now())
            .await
            .unwrap();
        let s = &statuses[0];
        assert_eq!((s.device_id, s.ignition), (7, IgnitionState::Unknown));
        assert_eq!(s.source, TrackingSource::Unavailable);
        assert!(!s.trustworthy, "unknown is stated, never guessed");
        assert!(s.position.is_none());
    }

    #[tokio::test]
    async fn a_failing_event_report_degrades_only_the_devices_that_needed_it() {
        // Device 1 answered from /positions; device 2 needed the report, which
        // is down. Failing the whole batch would throw away a good reading.
        let provider = FakeProvider::new(vec![
            position(1, 10, Some(true)),
            position(2, 9_999, Some(true)),
        ])
        .with_failing_events();
        let statuses = VehicleTrackingUseCase::new(provider)
            .current_status(&[1, 2], now())
            .await
            .expect("the batch survives a failing events report");

        assert!(
            statuses[0].supports_presence_claim(),
            "device 1 is untouched"
        );
        assert_eq!(statuses[1].source, TrackingSource::Unavailable);
        assert!(!statuses[1].trustworthy);
        assert!(
            statuses[1].position.is_some(),
            "its last known coordinate survives the failure"
        );
    }

    #[tokio::test]
    async fn a_failing_position_feed_fails_the_batch() {
        // Nothing to degrade *from* -- unlike the events report, there is no
        // partial answer worth returning.
        assert!(
            VehicleTrackingUseCase::new(DeadProvider)
                .current_status(&[1], now())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn the_newest_event_wins() {
        let provider = FakeProvider::new(Vec::new()).with_events(vec![
            event(1, 100, false),
            event(1, 10, true),
            event(1, 50, false),
        ]);
        let statuses = VehicleTrackingUseCase::new(provider)
            .current_status(&[1], now())
            .await
            .unwrap();
        assert_eq!(statuses[0].ignition, IgnitionState::On);
        assert_eq!(statuses[0].reported_at, Some(now() - Duration::seconds(10)));
    }

    #[tokio::test]
    async fn an_empty_request_asks_the_provider_nothing() {
        let use_case = VehicleTrackingUseCase::new(FakeProvider::new(Vec::new()));
        assert!(
            use_case
                .current_status(&[], now())
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            use_case.provider.seen_scope.lock().unwrap().is_none(),
            "no devices, no call"
        );
    }

    #[tokio::test]
    async fn every_requested_device_gets_exactly_one_answer_in_order() {
        let provider = FakeProvider::new(vec![position(2, 10, Some(true))]);
        let statuses = VehicleTrackingUseCase::new(provider)
            .current_status(&[3, 2, 1], now())
            .await
            .unwrap();
        assert_eq!(
            statuses.iter().map(|s| s.device_id).collect::<Vec<_>>(),
            vec![3, 2, 1]
        );
    }

    #[tokio::test]
    async fn ingestion_runs_under_a_named_tenant_scope_not_the_whole_platform() {
        // HRMS-903 / U-016. Before `run_for_tenant`, the only way out of
        // D-05's `Denied` default was `run_as_platform`, which would have
        // given this poller the entire platform for free.
        let use_case =
            VehicleTrackingUseCase::new(FakeProvider::new(vec![position(1, 10, Some(true))]));
        use_case
            .ingest_for_tenant(42, &[1])
            .await
            .expect("ingested");

        assert_eq!(
            *use_case.provider.seen_scope.lock().unwrap(),
            Some(TenantScope::Tenant(42))
        );
    }

    #[tokio::test]
    async fn ingestion_does_not_leave_a_scope_behind_it() {
        let use_case = VehicleTrackingUseCase::new(FakeProvider::new(Vec::new()));
        use_case
            .ingest_for_tenant(42, &[1])
            .await
            .expect("ingested");
        assert_eq!(tenant_scope(), TenantScope::Denied);
    }
}
