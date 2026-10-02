use business::domain::business_error::BusinessError;
use business::domain::enums::{PresenceEventKind, VehicleStatus};
use business::domain::garage_call::GarageCall;
use business::domain::vehicle::Vehicle;
use business::domain::vehicle_presence_event::VehiclePresenceEvent;
use business::gateway::garage_call_gateway::GarageCallStore;
use business::gateway::vehicle_gateway::VehicleLookupStore;
use business::gateway::vehicle_presence_event_gateway::VehiclePresenceStore;
use business::use_cases::garage_call_use_case::{
    GarageCallUseCase, NO_ACTIVE_CALL, NOT_AWAY, VEHICLE_NOT_FOUND,
};
use chrono::{NaiveDate, NaiveDateTime};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryCallStore {
    calls: Mutex<Vec<GarageCall>>,
}

#[async_trait]
impl GarageCallStore for MemoryCallStore {
    async fn save(&self, mut call: GarageCall) -> Result<GarageCall, BusinessError> {
        let mut calls = self
            .calls
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let id = call.id.unwrap_or(calls.len() as i64 + 1);
        call.id = Some(id);
        call.uuid = Some(format!("garage-call-{id}"));
        if let Some(existing) = calls.iter_mut().find(|existing| existing.id == Some(id)) {
            *existing = call.clone();
        } else {
            calls.push(call.clone());
        }
        Ok(call)
    }

    async fn get_latest_uncancelled(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<GarageCall>, BusinessError> {
        let calls = self
            .calls
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(calls
            .iter()
            .filter(|call| call.vehicle_id == vehicle_id && call.cancelled_at.is_none())
            .max_by_key(|call| (call.called_at, call.id))
            .cloned())
    }
}

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

    async fn save(&self, event: VehiclePresenceEvent) -> Result<(), BusinessError> {
        self.events
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?
            .push(event);
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

fn at(hour: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 10, 2)
        .expect("valid test date")
        .and_hms_opt(hour, 0, 0)
        .expect("valid test time")
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

fn event(kind: PresenceEventKind, hour: u32) -> VehiclePresenceEvent {
    VehiclePresenceEvent {
        id: Some(hour as i64),
        uuid: Some(format!("event-{hour}")),
        tenant_id: Some(42),
        vehicle_id: 10,
        kind,
        occurred_at: at(hour),
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn use_case(events: Vec<VehiclePresenceEvent>) -> GarageCallUseCase {
    GarageCallUseCase::new(
        MemoryCallStore::default(),
        MemoryPresenceStore {
            events: Mutex::new(events),
        },
        MemoryVehicleStore {
            vehicle: Some(vehicle()),
        },
    )
}

#[tokio::test]
async fn an_away_vehicle_can_be_called_and_repeated_calls_are_not_stamped_again() {
    let use_case = use_case(vec![event(PresenceEventKind::Departure, 8)]);

    let (_, created) = use_case
        .call(Some(42), 10, Some(3))
        .await
        .expect("away vehicle can be called");
    let (existing, repeated) = use_case
        .call(Some(42), 10, Some(3))
        .await
        .expect("active call is returned");

    assert!(created);
    assert!(!repeated);
    assert_eq!(existing.vehicle_id, 10);
}

#[tokio::test]
async fn a_vehicle_arrived_at_base_cannot_be_called() {
    let use_case = use_case(vec![
        event(PresenceEventKind::Departure, 8),
        event(PresenceEventKind::Arrival, 9),
    ]);

    let error = use_case
        .call(Some(42), 10, None)
        .await
        .expect_err("vehicle is no longer away");

    assert_eq!(error.message, NOT_AWAY);
}

#[tokio::test]
async fn calls_cannot_be_made_for_a_vehicle_in_another_tenant() {
    let mut other_tenant = vehicle();
    other_tenant.tenant_id = Some(99);
    let use_case = GarageCallUseCase::new(
        MemoryCallStore::default(),
        MemoryPresenceStore {
            events: Mutex::new(vec![event(PresenceEventKind::Departure, 8)]),
        },
        MemoryVehicleStore {
            vehicle: Some(other_tenant),
        },
    );

    let error = use_case
        .call(Some(42), 10, None)
        .await
        .expect_err("tenant mismatch is hidden as not found");

    assert_eq!(error.message, VEHICLE_NOT_FOUND);
}

#[tokio::test]
async fn a_call_can_be_cancelled_and_then_is_no_longer_active() {
    let use_case = use_case(vec![event(PresenceEventKind::Departure, 8)]);
    let (call, _) = use_case
        .call(Some(42), 10, None)
        .await
        .expect("call is created");

    let cancelled = use_case.cancel(10).await.expect("call is cancelled");
    let error = use_case
        .cancel(10)
        .await
        .expect_err("no uncancelled call remains");

    assert!(cancelled.cancelled_at.is_some());
    assert_eq!(cancelled.id, call.id);
    assert_eq!(error.message, NO_ACTIVE_CALL);
}
