use business::domain::business_error::BusinessError;
use business::domain::enums::{
    GarageAttendanceOrigin, GarageAttendanceStatus, GarageServiceGroup, GarageServiceState,
    PresenceEventKind,
};
use business::domain::extra_trip::ExtraTrip;
use business::domain::garage_attendance::GarageAttendance;
use business::domain::garage_service::GarageService;
use business::domain::garage_service_model::{GarageServiceApplicability, GarageServiceModel};
use business::domain::tenant_rule_setting::TenantRuleSetting;
use business::domain::vehicle_presence_event::VehiclePresenceEvent;
use business::gateway::extra_trip_gateway::GarageValidityTripStore;
use business::gateway::garage_service_model_gateway::GarageServiceModelStore;
use business::gateway::tenant_rule_setting_gateway::TenantRuleSettingStore;
use business::gateway::vehicle_presence_event_gateway::GarageValidityPresenceStore;
use business::use_cases::fuel_gauge_use_case::{FuelGaugeReader, GaugeUnavailable, TankGauge};
use business::use_cases::garage_validity::{TankFact, ValidityReason};
use business::use_cases::garage_validity_use_case::GarageValidityUseCase;
use chrono::{Duration, NaiveDate, NaiveDateTime};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct MemoryPresenceStore {
    events: Vec<VehiclePresenceEvent>,
}

#[async_trait]
impl GarageValidityPresenceStore for MemoryPresenceStore {
    async fn latest_of_kind(
        &self,
        vehicle_id: i64,
        kind: PresenceEventKind,
    ) -> Result<Option<VehiclePresenceEvent>, BusinessError> {
        Ok(self
            .events
            .iter()
            .filter(|event| event.vehicle_id == vehicle_id && event.kind == kind)
            .max_by_key(|event| (event.occurred_at, event.id))
            .cloned())
    }

    async fn latest_of_kind_before(
        &self,
        vehicle_id: i64,
        kind: PresenceEventKind,
        before: NaiveDateTime,
    ) -> Result<Option<VehiclePresenceEvent>, BusinessError> {
        Ok(self
            .events
            .iter()
            .filter(|event| {
                event.vehicle_id == vehicle_id && event.kind == kind && event.occurred_at < before
            })
            .max_by_key(|event| (event.occurred_at, event.id))
            .cloned())
    }
}

#[derive(Default)]
struct MemoryTripStore;

#[async_trait]
impl GarageValidityTripStore for MemoryTripStore {
    async fn upcoming_for_vehicle(
        &self,
        _vehicle_id: i64,
        _from: NaiveDate,
    ) -> Result<Vec<ExtraTrip>, BusinessError> {
        Ok(Vec::new())
    }
}

#[derive(Default)]
struct MemoryServiceModelStore {
    models: Vec<GarageServiceModel>,
}

#[async_trait]
impl GarageServiceModelStore for MemoryServiceModelStore {
    async fn save(&self, model: GarageServiceModel) -> Result<GarageServiceModel, BusinessError> {
        Ok(model)
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<GarageServiceModel>, BusinessError> {
        Ok(self
            .models
            .iter()
            .find(|model| model.id == Some(id))
            .cloned())
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<GarageServiceModel>, BusinessError> {
        Ok(self
            .models
            .iter()
            .find(|model| model.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }

    async fn find_by_key(
        &self,
        name_key: &str,
        tenant_id: Option<i64>,
    ) -> Result<Option<GarageServiceModel>, BusinessError> {
        Ok(self
            .models
            .iter()
            .find(|model| model.name_key == name_key && model.tenant_id == tenant_id)
            .cloned())
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<GarageServiceModel>, u64), BusinessError> {
        let total = self.models.len() as u64;
        let start = (page * page_size) as usize;
        Ok((
            self.models
                .iter()
                .skip(start)
                .take(page_size as usize)
                .cloned()
                .collect(),
            total,
        ))
    }
}

#[derive(Default)]
struct MemoryRuleStore;

#[async_trait]
impl TenantRuleSettingStore for MemoryRuleStore {
    async fn get_current(
        &self,
        _tenant_id: Option<i64>,
    ) -> Result<Option<TenantRuleSetting>, BusinessError> {
        Ok(None)
    }

    async fn save(&self, setting: TenantRuleSetting) -> Result<TenantRuleSetting, BusinessError> {
        Ok(setting)
    }
}

#[derive(Clone)]
struct FakeGaugeReader {
    calls: Arc<Mutex<usize>>,
    percent: Option<f64>,
}

#[async_trait]
impl FuelGaugeReader for FakeGaugeReader {
    async fn read_gauge(&self, _vehicle_id: i64) -> Result<TankGauge, BusinessError> {
        *self
            .calls
            .lock()
            .map_err(|_| BusinessError::new("gauge counter poisoned".into()))? += 1;
        Ok(match self.percent {
            Some(percent) => TankGauge::Reading {
                percent,
                remaining_liters: 18.5,
                range_km: 120.0,
                consumption_km_per_liter: 6.5,
                anchored_at: chrono::Utc::now().naive_utc(),
                estimated: false,
                set_aside_at: Vec::new(),
            },
            None => TankGauge::Unavailable(GaugeUnavailable::NoTankRegistered),
        })
    }
}

fn attendance(status: GarageAttendanceStatus) -> GarageAttendance {
    GarageAttendance {
        id: Some(1),
        uuid: Some("attendance-1".into()),
        tenant_id: Some(42),
        vehicle_id: 10,
        attendance_date: NaiveDate::from_ymd_opt(2026, 10, 2).expect("valid test date"),
        checked_in_at: NaiveDate::from_ymd_opt(2026, 10, 2)
            .expect("valid test date")
            .and_hms_opt(8, 0, 0)
            .expect("valid test time"),
        status,
        manual_priority: None,
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

fn service(state: GarageServiceState) -> GarageService {
    GarageService {
        id: Some(1),
        uuid: Some("service-1".into()),
        tenant_id: Some(42),
        attendance_id: 1,
        service_model_id: 7,
        name_key: "brakes".into(),
        state,
        performed_at: None,
        marked_at: None,
        forced_pending_at: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn presence_event(
    id: i64,
    kind: PresenceEventKind,
    occurred_at: NaiveDateTime,
) -> VehiclePresenceEvent {
    VehiclePresenceEvent {
        id: Some(id),
        uuid: Some(format!("presence-{id}")),
        tenant_id: Some(42),
        vehicle_id: 10,
        kind,
        occurred_at,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn model(governed_by_tank: bool, required_for_departure: bool) -> GarageServiceModel {
    GarageServiceModel {
        id: Some(7),
        uuid: Some("model-7".into()),
        tenant_id: Some(42),
        name: "Brake inspection".into(),
        name_key: "brakes".into(),
        display_order: 1,
        active: true,
        service_group: GarageServiceGroup::External,
        required_for_departure,
        governed_by_tank,
        applicability: GarageServiceApplicability::All,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn use_case(models: Vec<GarageServiceModel>) -> (GarageValidityUseCase, Arc<Mutex<usize>>) {
    use_case_with_gauge(models, None)
}

fn use_case_with_gauge(
    models: Vec<GarageServiceModel>,
    percent: Option<f64>,
) -> (GarageValidityUseCase, Arc<Mutex<usize>>) {
    use_case_with_events(models, percent, Vec::new())
}

fn use_case_with_events(
    models: Vec<GarageServiceModel>,
    percent: Option<f64>,
    events: Vec<VehiclePresenceEvent>,
) -> (GarageValidityUseCase, Arc<Mutex<usize>>) {
    let gauge_calls = Arc::new(Mutex::new(0));
    let use_case = GarageValidityUseCase::new(
        MemoryPresenceStore { events },
        MemoryTripStore,
        MemoryServiceModelStore { models },
        MemoryRuleStore,
        FakeGaugeReader {
            calls: gauge_calls.clone(),
            percent,
        },
    );
    (use_case, gauge_calls)
}

#[tokio::test]
async fn closed_attendance_preserves_stored_states_without_reading_other_ports() {
    let (use_case, gauge_calls) = use_case(Vec::new());
    let result = use_case
        .evaluate_full(
            &attendance(GarageAttendanceStatus::Finished),
            &[service(GarageServiceState::Performed)],
        )
        .await
        .expect("closed attendance is history");

    assert_eq!(
        result.services,
        vec![(GarageServiceState::Performed, ValidityReason::Valid)]
    );
    assert_eq!(*gauge_calls.lock().expect("gauge counter lock"), 0);
}

#[tokio::test]
async fn open_attendance_reads_service_metadata_without_gauge_when_not_tank_governed() {
    let (use_case, gauge_calls) = use_case(vec![model(false, true)]);
    let result = use_case
        .evaluate_full(
            &attendance(GarageAttendanceStatus::Open),
            &[service(GarageServiceState::Pending)],
        )
        .await
        .expect("open attendance is evaluated");

    assert_eq!(result.required, vec![true]);
    assert_eq!(result.names, vec!["Brake inspection"]);
    assert_eq!(result.services[0].0, GarageServiceState::Pending);
    assert_eq!(*gauge_calls.lock().expect("gauge counter lock"), 0);
}

#[tokio::test]
async fn tank_governed_services_read_the_gauge_and_expose_the_tank_fact() {
    let (use_case, gauge_calls) = use_case_with_gauge(vec![model(true, true)], Some(37.0));
    let result = use_case
        .evaluate_full(
            &attendance(GarageAttendanceStatus::Open),
            &[service(GarageServiceState::Pending)],
        )
        .await
        .expect("tank-governed service is evaluated");

    assert_eq!(result.required, vec![true]);
    assert_eq!(result.tank, TankFact::Percent(37.0));
    assert_eq!(*gauge_calls.lock().unwrap(), 1);

    assert_eq!(
        use_case.rules_for(Some(42)).await.unwrap(),
        Default::default()
    );
    assert_eq!(
        use_case.tank_fact(10).await,
        TankFact::Percent(37.0),
        "the monitor can request tank data independently of services"
    );
    assert_eq!(*gauge_calls.lock().unwrap(), 2);
}

#[tokio::test]
async fn presence_events_consume_marks_and_trip_returns_reset_performed_services() {
    let marked_at = attendance(GarageAttendanceStatus::Open).checked_in_at;
    let after_mark = marked_at + Duration::hours(1);
    let (departure_case, _) = use_case_with_events(
        vec![model(false, true)],
        None,
        vec![presence_event(1, PresenceEventKind::Departure, after_mark)],
    );
    let mut marked_service = service(GarageServiceState::NotDone);
    marked_service.marked_at = Some(marked_at);
    let departure_result = departure_case
        .evaluate_full(
            &attendance(GarageAttendanceStatus::Open),
            &[marked_service.clone()],
        )
        .await
        .expect("departure consumes the mark");
    assert_eq!(
        departure_result.services[0],
        (
            GarageServiceState::Pending,
            ValidityReason::ConsumedByDeparture
        )
    );

    let arrival_case = use_case_with_events(
        vec![model(false, true)],
        None,
        vec![presence_event(
            2,
            PresenceEventKind::Arrival,
            after_mark + Duration::hours(1),
        )],
    )
    .0;
    let arrival_result = arrival_case
        .evaluate_full(&attendance(GarageAttendanceStatus::Open), &[marked_service])
        .await
        .expect("arrival after the mark invalidates it");
    assert_eq!(
        arrival_result.services[0],
        (GarageServiceState::Pending, ValidityReason::ReturnedToBase)
    );

    let trip_departure = after_mark + Duration::hours(1);
    let trip_arrival = trip_departure + Duration::days(1);
    let trip_case = use_case_with_events(
        vec![model(false, true)],
        None,
        vec![
            presence_event(3, PresenceEventKind::Departure, trip_departure),
            presence_event(4, PresenceEventKind::Arrival, trip_arrival),
        ],
    )
    .0;
    let mut performed_service = service(GarageServiceState::Performed);
    performed_service.performed_at = Some(marked_at);
    let trip_result = trip_case
        .evaluate_full(
            &attendance(GarageAttendanceStatus::Open),
            &[performed_service],
        )
        .await
        .expect("a completed trip resets the service");
    assert_eq!(
        trip_result.services[0],
        (GarageServiceState::Pending, ValidityReason::TripReturn)
    );
}
