use business::domain::business_error::BusinessError;
use business::domain::enums::{PresenceEventKind, PresenceSource, VehicleStatus};
use business::domain::vehicle::Vehicle;
use business::domain::vehicle_presence_event::VehiclePresenceEvent;
use business::gateway::vehicle_gateway::VehicleLookupStore;
use business::gateway::vehicle_presence_event_gateway::VehiclePresenceStore;
use business::use_cases::vehicle_presence_use_case::{
    ESTIMATE_CANNOT_STAMP, VehiclePresenceUseCase,
};
use chrono::Utc;
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryPresenceStore {
    events: Mutex<Vec<VehiclePresenceEvent>>,
}

#[async_trait]
impl VehiclePresenceStore for MemoryPresenceStore {
    async fn get_latest(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<VehiclePresenceEvent>, BusinessError> {
        let events = self
            .events
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(events
            .iter()
            .filter(|event| event.vehicle_id == vehicle_id)
            .max_by_key(|event| (event.occurred_at, event.id))
            .cloned())
    }

    async fn get_latest_of_kind(
        &self,
        vehicle_id: i64,
        kind: PresenceEventKind,
    ) -> Result<Option<VehiclePresenceEvent>, BusinessError> {
        let events = self
            .events
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(events
            .iter()
            .filter(|event| event.vehicle_id == vehicle_id && event.kind == kind)
            .max_by_key(|event| (event.occurred_at, event.id))
            .cloned())
    }

    async fn save(&self, mut event: VehiclePresenceEvent) -> Result<(), BusinessError> {
        let mut events = self
            .events
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        event.id = Some(events.len() as i64 + 1);
        events.push(event);
        Ok(())
    }
}

struct MemoryVehicleStore {
    vehicle: Option<Vehicle>,
}

#[async_trait]
impl VehicleLookupStore for MemoryVehicleStore {
    async fn get_by_id(&self, id: i64) -> Result<Option<Vehicle>, BusinessError> {
        Ok(self
            .vehicle
            .as_ref()
            .filter(|vehicle| vehicle.id == Some(id))
            .cloned())
    }
}

fn vehicle() -> Vehicle {
    Vehicle {
        id: Some(10),
        uuid: Some("vehicle-10".into()),
        tenant_id: Some(42),
        plate: "ABC1D23".into(),
        model: "Truck".into(),
        status: VehicleStatus::Active,
        tracker_device_id: None,
        prefix: None,
        vehicle_type: None,
        odometer_km: None,
        wheel_type: None,
        spare_tire_count: None,
        spare_tire_type: None,
        spare_tire_notes: None,
        tank_capacity_liters: None,
        reference_km_per_liter: None,
        garage_tag: None,
        garage_tag_origin: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn use_case() -> VehiclePresenceUseCase {
    VehiclePresenceUseCase::new(
        MemoryPresenceStore::default(),
        MemoryVehicleStore {
            vehicle: Some(vehicle()),
        },
    )
}

#[tokio::test]
async fn tracker_arrival_and_departure_update_the_snapshot_in_order() {
    let use_case = use_case();
    let now = Utc::now().naive_utc();

    let (arrival_recorded, arrival) = use_case
        .record(
            Some(42),
            10,
            PresenceEventKind::Arrival,
            now - chrono::Duration::seconds(30),
            PresenceSource::Tracker,
        )
        .await
        .expect("arrival is recorded");
    let (departure_recorded, departure) = use_case
        .record(
            Some(42),
            10,
            PresenceEventKind::Departure,
            now - chrono::Duration::seconds(10),
            PresenceSource::Tracker,
        )
        .await
        .expect("departure is recorded");

    assert!(arrival_recorded);
    assert!(!arrival.away);
    assert!(departure_recorded);
    assert!(departure.away);
    assert!(departure.last_departure_at > departure.last_arrival_at);
}

#[tokio::test]
async fn repeated_arrival_is_ignored_and_does_not_move_the_snapshot() {
    let use_case = use_case();
    let now = Utc::now().naive_utc();
    let first = now - chrono::Duration::seconds(20);
    use_case
        .record(
            Some(42),
            10,
            PresenceEventKind::Arrival,
            first,
            PresenceSource::Tracker,
        )
        .await
        .expect("first arrival is recorded");

    let (recorded, snapshot) = use_case
        .record(
            Some(42),
            10,
            PresenceEventKind::Arrival,
            now - chrono::Duration::seconds(5),
            PresenceSource::Tracker,
        )
        .await
        .expect("repeated arrival is ignored");

    assert!(!recorded);
    assert_eq!(snapshot.last_arrival_at, Some(first));
}

#[tokio::test]
async fn schedule_estimates_are_rejected_before_storage() {
    let use_case = use_case();
    let error = use_case
        .record(
            Some(42),
            10,
            PresenceEventKind::Departure,
            Utc::now().naive_utc(),
            PresenceSource::ScheduleEstimate,
        )
        .await
        .expect_err("only tracker events stamp presence");

    assert_eq!(error.message, ESTIMATE_CANNOT_STAMP);
}
