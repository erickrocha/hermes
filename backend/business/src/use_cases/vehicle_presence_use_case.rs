use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::{PresenceEventKind, PresenceSource};
use crate::domain::vehicle_presence_event::{VehiclePresenceEvent, VehiclePresenceEventEntityMapper};
use crate::gateway::vehicle_gateway::VehicleGateway;
use crate::gateway::vehicle_presence_event_gateway::VehiclePresenceEventGateway;
use chrono::NaiveDateTime;
use sea_orm::DbErr;

pub const VEHICLE_NOT_FOUND: &str = "Vehicle not found";
/// `D-24(e)`: only a real tracker reading stamps presence.
pub const ESTIMATE_CANNOT_STAMP: &str = "A schedule estimate cannot stamp a vehicle's arrival or departure";
pub const OUT_OF_ORDER: &str = "This event is earlier than the vehicle's latest recorded arrival or departure";
pub const IN_THE_FUTURE: &str = "A presence event cannot be in the future";

/// How far ahead of the server clock a tracker's timestamp may be (clock skew).
const SKEW_SECONDS: i64 = 300;

/// What the vehicle's physical stamps say now (`TRM-788`).
#[derive(Debug, Clone, PartialEq)]
pub struct PresenceSnapshot {
    pub last_arrival_at: Option<NaiveDateTime>,
    pub last_departure_at: Option<NaiveDateTime>,
    /// The latest event is a departure: the vehicle is away from the base.
    pub away: bool,
}

/// `EPIC-GA-03-S01` (`HRMS-960`): the vehicle's physical arrival and departure
/// stamps -- the presence domain's exclusive output (`AD-036`, `TRM-788`).
pub struct VehiclePresenceUseCase {
    events: VehiclePresenceEventGateway,
    vehicles: VehicleGateway,
}

impl VehiclePresenceUseCase {
    pub fn new(events: VehiclePresenceEventGateway, vehicles: VehicleGateway) -> Self {
        Self { events, vehicles }
    }

    /// `TRM-770…775`: records a physical arrival or departure. An event is new
    /// only when it alternates with the latest one -- a departure without an
    /// arrival in between means the vehicle never came back, and a second
    /// arrival that no departure separated is the same arrival re-reported; both
    /// leave the stamps untouched (returns `false`). Only a real tracker reading
    /// may stamp (`D-24(e)`), and events may not go backwards.
    pub async fn record(
        &self,
        tenant_id: Option<i64>,
        vehicle_id: i64,
        kind: PresenceEventKind,
        occurred_at: NaiveDateTime,
        source: PresenceSource,
    ) -> Result<(bool, PresenceSnapshot), BusinessError> {
        if source != PresenceSource::Tracker {
            return Err(BusinessError::new(ESTIMATE_CANNOT_STAMP.to_string()));
        }
        if occurred_at > chrono::Utc::now().naive_utc() + chrono::Duration::seconds(SKEW_SECONDS) {
            return Err(BusinessError::new(IN_THE_FUTURE.to_string()));
        }
        let vehicle = self.vehicles.find_by_id(vehicle_id).await.map_err(database_error)?;
        if !vehicle.is_some_and(|v| v.tenant_id == tenant_id) {
            return Err(BusinessError::new(VEHICLE_NOT_FOUND.to_string()));
        }

        let latest = self
            .events
            .find_latest(vehicle_id)
            .await
            .map_err(database_error)?
            .map(VehiclePresenceEventEntityMapper::from_model);
        let decision = decide(latest.as_ref().map(|e| (e.kind, e.occurred_at)), kind, occurred_at)?;
        let recorded = decision == Decision::Record;
        if recorded {
            self.events
                .persist(VehiclePresenceEvent {
                    id: None,
                    uuid: None,
                    tenant_id,
                    vehicle_id,
                    kind,
                    occurred_at,
                    created_at: None,
                    created_by: None,
                    updated_at: None,
                    updated_by: None,
                })
                .await
                .map_err(database_error)?;
        }
        Ok((recorded, self.snapshot(vehicle_id).await?))
    }

    pub async fn snapshot(&self, vehicle_id: i64) -> Result<PresenceSnapshot, BusinessError> {
        let at = |model: Option<entity::vehicle_presence_event_entity::Model>| model.map(|m| m.occurred_at.naive_utc());
        let last_arrival_at = at(self.events.find_latest_of_kind(vehicle_id, "Arrival").await.map_err(database_error)?);
        let last_departure_at = at(self.events.find_latest_of_kind(vehicle_id, "Departure").await.map_err(database_error)?);
        let away = match (last_arrival_at, last_departure_at) {
            (_, None) => false,
            (None, Some(_)) => true,
            (Some(arrival), Some(departure)) => departure > arrival,
        };
        Ok(PresenceSnapshot { last_arrival_at, last_departure_at, away })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Decision {
    Record,
    /// The vehicle is already in that state: the stamps stay as they are.
    Ignore,
}

/// `TRM-770`: the alternation rule, with `TRM-774`'s recovery falling out of it
/// (a departure after a later arrival is new). Events may not go backwards.
pub fn decide(
    latest: Option<(PresenceEventKind, NaiveDateTime)>,
    kind: PresenceEventKind,
    at: NaiveDateTime,
) -> Result<Decision, BusinessError> {
    match latest {
        None => Ok(Decision::Record),
        Some((_, previous_at)) if at < previous_at => Err(BusinessError::new(OUT_OF_ORDER.to_string())),
        Some((previous, _)) if previous == kind => Ok(Decision::Ignore),
        Some(_) => Ok(Decision::Record),
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[VehiclePresenceUseCase] {}", msg);
    BusinessError::new(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(h: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, 30).unwrap().and_hms_opt(h, 0, 0).unwrap()
    }

    #[test]
    fn a_departure_is_new_only_when_an_arrival_came_in_between() {
        use PresenceEventKind::*;
        assert_eq!(decide(None, Departure, at(8)), Ok(Decision::Record), "the first event is new");
        assert_eq!(decide(Some((Arrival, at(7))), Departure, at(8)), Ok(Decision::Record));
        // The vehicle never came back: a second departure leaves the stamp alone.
        assert_eq!(decide(Some((Departure, at(8))), Departure, at(9)), Ok(Decision::Ignore));
    }

    #[test]
    fn an_arrival_is_new_only_when_a_departure_preceded_it() {
        use PresenceEventKind::*;
        assert_eq!(decide(Some((Departure, at(8))), Arrival, at(9)), Ok(Decision::Record));
        // Re-reporting an arrival must not move the stamp later: a later arrival would wrongly
        // return every not-needed mark to pending (TRM-455).
        assert_eq!(decide(Some((Arrival, at(7))), Arrival, at(9)), Ok(Decision::Ignore));
    }

    #[test]
    fn an_event_may_not_go_backwards() {
        use PresenceEventKind::*;
        assert_eq!(decide(Some((Departure, at(8))), Arrival, at(7)), Err(BusinessError::new(OUT_OF_ORDER.to_string())));
    }
}
