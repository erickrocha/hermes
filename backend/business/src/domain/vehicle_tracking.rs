//! Vehicle tracking — the provider-agnostic domain vocabulary (EPIC-FT-01).
//!
//! `PD-025` decided hermes **consumes** an existing GPS provider rather than
//! rebuilding `operacao-trm`'s `rastreador-cron` polling tracker. `D-15` named
//! the provider (PinME, a Traccar-compatible API). Nothing in this module names
//! it: the provider's wire format is confined to
//! `gateway::tracking_provider` (`HRMS-902`), and everything above that
//! boundary speaks only the types declared here.
//!
//! Every name is English (`HRMS-904`/`AD-019`) even though the reference
//! implementation this behaviour is inherited from is written in Portuguese
//! (`ignicao_status`, `confiavel`, `fonte`). Translating at the boundary is the
//! point — the first fleet-ops concept on hermes does not import
//! `operacao-trm`'s naming.
//!
//! **Why every reading carries provenance, quality and an instant.** `PD-025`'s
//! note to whoever resolves `operacao-trm`'s `D-01` argues that a third-party
//! source makes "fuel level and presence as explicit, provenanced inputs"
//! materially easier, because the payload *arrives* as an input with an origin
//! and a timestamp instead of being derived and consumed by the same
//! automation. These structs are where that promise is kept or lost.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// How much of the vehicle's state the provider was able to tell us.
///
/// `Unknown` is a first-class answer, not an error and not a default. The
/// production rule this integration inherits is that a vehicle whose state
/// cannot be established is reported as unknown rather than guessed.
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum IgnitionState {
    On,
    Off,
    #[default]
    Unknown,
}

/// Where a reading came from. Provenance is carried, never inferred by the
/// consumer: `LastPosition` and `IgnitionEvent` are different qualities of
/// evidence and a consumer is entitled to treat them differently.
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum TrackingSource {
    /// The provider's current position feed — the preferred source.
    LastPosition,
    /// The ignition-event report, consulted when the position is stale,
    /// invalid, or carries no ignition attribute.
    IgnitionEvent,
    /// Neither source could answer for this device.
    #[default]
    Unavailable,
}

/// A coordinate as the provider reported it.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize, ToSchema)]
pub struct GeoPoint {
    pub latitude: f64,
    pub longitude: f64,
}

/// One vehicle's tracking state, as of one instant, from one named source.
///
/// This is the *only* type the ingestion boundary hands upwards. It is
/// deliberately not an entity and has no table behind it — `HRMS-905` requires
/// the consumer to be stated before tracking data is persisted, and that
/// consumer is `operacao-trm`'s undecided `D-01`. See the module note on
/// `use_cases::vehicle_tracking_use_case`.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, ToSchema)]
pub struct VehicleTrackingStatus {
    /// The provider's identifier for the device. Hermes holds no vehicle
    /// entity to map it onto — that arrives with the fleet-ops domain.
    pub device_id: i64,
    pub ignition: IgnitionState,
    pub source: TrackingSource,
    /// When the evidence behind `ignition` was recorded by the provider, not
    /// when hermes fetched it. `None` only when `source` is `Unavailable`.
    pub reported_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Whether this reading may be relied on. A stale, invalid or
    /// event-derived reading is still *returned* — with `trustworthy` false —
    /// because a consumer may reasonably display a last-known state as long as
    /// it knows that is what it is.
    pub trustworthy: bool,
    /// The last coordinate the provider reported, attached even when the
    /// reading is stale: a parked vehicle stops emitting but has not moved,
    /// and a garage view still needs somewhere to draw it.
    pub position: Option<GeoPoint>,
    /// Accumulated distance in metres, when the provider reported it. Named
    /// for what it is rather than for the fuel gauge that consumes it
    /// downstream — that consumer is `D-01`'s to decide.
    pub odometer_meters: Option<f64>,
}

impl VehicleTrackingStatus {
    /// The answer for a device nothing could be established about.
    ///
    /// Deliberately not `Default::default()`: constructing an unknown status
    /// should look like a decision at the call site, because it is one.
    pub fn unknown(device_id: i64) -> Self {
        Self {
            device_id,
            ignition: IgnitionState::Unknown,
            source: TrackingSource::Unavailable,
            reported_at: None,
            trustworthy: false,
            position: None,
            odometer_meters: None,
        }
    }

    /// `true` when this reading is strong enough to support a claim about
    /// where the vehicle *is* — as opposed to where it was last seen.
    ///
    /// The rule exists because of two production incidents recorded in
    /// `operacao-trm`'s own notes: a stale reading must never leave a vehicle
    /// tagged as being at base. Consumers are expected to ask this rather than
    /// re-derive it from `trustworthy` and `source`, so the rule lives in one
    /// place.
    pub fn supports_presence_claim(&self) -> bool {
        self.trustworthy && self.source == TrackingSource::LastPosition
    }
}
