use business::domain::business_error::BusinessError;
use business::domain::enums::{
    GarageAttendanceOrigin, GarageAttendanceStatus, GarageServiceGroup, GarageServiceState,
    PresenceEventKind,
};
use business::domain::extra_trip::ExtraTrip;
use business::domain::garage_attendance::GarageAttendance;
use business::domain::garage_call::GarageCall;
use business::domain::garage_service::GarageService;
use business::domain::garage_service_model::{GarageServiceApplicability, GarageServiceModel};
use business::domain::tenant_rule_setting::TenantRuleSetting;
use business::domain::vehicle::Vehicle;
use business::domain::vehicle_presence_event::VehiclePresenceEvent;
use business::gateway::effective_schedule_gateway::EffectiveScheduleStore;
use business::gateway::extra_trip_gateway::GarageValidityTripStore;
use business::gateway::garage_call_gateway::GarageCallStore;
use business::gateway::garage_queue_gateway::GarageQueueStore;
use business::gateway::garage_service_model_gateway::GarageServiceModelStore;
use business::gateway::tenant_rule_setting_gateway::TenantRuleSettingStore;
use business::gateway::vehicle_gateway::VehicleLookupStore;
use business::gateway::vehicle_presence_event_gateway::{
    GarageValidityPresenceStore, VehiclePresenceStore,
};
use business::use_cases::effective_schedule_use_case::EffectiveScheduleUseCase;
use business::use_cases::fuel_gauge_use_case::{FuelGaugeReader, GaugeUnavailable, TankGauge};
use business::use_cases::garage_call_use_case::GarageCallUseCase;
use business::use_cases::garage_queue_use_case::GarageQueueUseCase;
use business::use_cases::garage_validity_use_case::GarageValidityUseCase;
use chrono::{NaiveDate, NaiveDateTime, Utc};
use sea_orm::prelude::async_trait::async_trait;

struct QueueStore {
    attendances: Vec<GarageAttendance>,
    vehicles: Vec<Vehicle>,
    services: Vec<GarageService>,
    models: Vec<GarageServiceModel>,
}

#[async_trait]
impl GarageQueueStore for QueueStore {
    async fn active_attendances(&self) -> Result<Vec<GarageAttendance>, BusinessError> {
        Ok(self.attendances.clone())
    }

    async fn all_vehicles(&self) -> Result<Vec<Vehicle>, BusinessError> {
        Ok(self.vehicles.clone())
    }

    async fn find_vehicle(&self, vehicle_id: i64) -> Result<Option<Vehicle>, BusinessError> {
        Ok(self
            .vehicles
            .iter()
            .find(|vehicle| vehicle.id == Some(vehicle_id))
            .cloned())
    }

    async fn services(&self, attendance_id: i64) -> Result<Vec<GarageService>, BusinessError> {
        Ok(self
            .services
            .iter()
            .filter(|service| service.attendance_id == attendance_id)
            .cloned()
            .collect())
    }

    async fn all_service_models(&self) -> Result<Vec<GarageServiceModel>, BusinessError> {
        Ok(self.models.clone())
    }

    async fn active_service_models(&self) -> Result<Vec<GarageServiceModel>, BusinessError> {
        Ok(self.models.clone())
    }

    async fn attendance_ids_by_vehicle(&self, vehicle_id: i64) -> Result<Vec<i64>, BusinessError> {
        Ok(self
            .attendances
            .iter()
            .filter(|attendance| attendance.vehicle_id == vehicle_id)
            .filter_map(|attendance| attendance.id)
            .collect())
    }

    async fn performed_services_by_attendances(
        &self,
        attendance_ids: Vec<i64>,
    ) -> Result<Vec<GarageService>, BusinessError> {
        Ok(self
            .services
            .iter()
            .filter(|service| {
                attendance_ids.contains(&service.attendance_id)
                    && service.state == GarageServiceState::Performed
            })
            .cloned()
            .collect())
    }

    async fn latest_attendance(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<GarageAttendance>, BusinessError> {
        Ok(self
            .attendances
            .iter()
            .find(|attendance| attendance.vehicle_id == vehicle_id)
            .cloned())
    }
}

struct EmptyScheduleStore;

#[async_trait]
impl EffectiveScheduleStore for EmptyScheduleStore {
    async fn demands(
        &self,
    ) -> Result<Vec<business::domain::transport_demand::TransportDemand>, BusinessError> {
        Ok(Vec::new())
    }

    async fn allocations(
        &self,
    ) -> Result<
        Vec<business::domain::transport_demand_allocation::TransportDemandAllocation>,
        BusinessError,
    > {
        Ok(Vec::new())
    }

    async fn daily_entries(
        &self,
        _date: NaiveDate,
    ) -> Result<Vec<business::domain::daily_schedule::DailySchedule>, BusinessError> {
        Ok(Vec::new())
    }

    async fn exceptions(
        &self,
        _date: NaiveDate,
    ) -> Result<Vec<business::domain::schedule_exception::ScheduleException>, BusinessError> {
        Ok(Vec::new())
    }

    async fn trips(&self, _date: NaiveDate) -> Result<Vec<ExtraTrip>, BusinessError> {
        Ok(Vec::new())
    }

    async fn is_holiday(&self, _date: NaiveDate) -> Result<bool, BusinessError> {
        Ok(false)
    }

    async fn day_off_customers(&self, _date: NaiveDate) -> Result<Vec<i64>, BusinessError> {
        Ok(Vec::new())
    }
}

struct EmptyTripStore;

#[async_trait]
impl GarageValidityTripStore for EmptyTripStore {
    async fn upcoming_for_vehicle(
        &self,
        _vehicle_id: i64,
        _from: NaiveDate,
    ) -> Result<Vec<ExtraTrip>, BusinessError> {
        Ok(Vec::new())
    }
}

struct QueueModels(Vec<GarageServiceModel>);

#[async_trait]
impl GarageServiceModelStore for QueueModels {
    async fn save(&self, model: GarageServiceModel) -> Result<GarageServiceModel, BusinessError> {
        Ok(model)
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<GarageServiceModel>, BusinessError> {
        Ok(self.0.iter().find(|model| model.id == Some(id)).cloned())
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<GarageServiceModel>, BusinessError> {
        Ok(self
            .0
            .iter()
            .find(|model| model.uuid.as_deref() == Some(&uuid))
            .cloned())
    }

    async fn find_by_key(
        &self,
        key: &str,
        tenant_id: Option<i64>,
    ) -> Result<Option<GarageServiceModel>, BusinessError> {
        Ok(self
            .0
            .iter()
            .find(|model| model.name_key == key && model.tenant_id == tenant_id)
            .cloned())
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<GarageServiceModel>, u64), BusinessError> {
        let start = (page * page_size) as usize;
        Ok((
            self.0
                .iter()
                .skip(start)
                .take(page_size as usize)
                .cloned()
                .collect(),
            self.0.len() as u64,
        ))
    }
}

struct EmptyRuleStore;

#[async_trait]
impl TenantRuleSettingStore for EmptyRuleStore {
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

struct QueuePresence(VehiclePresenceEvent);

#[async_trait]
impl GarageValidityPresenceStore for QueuePresence {
    async fn latest_of_kind(
        &self,
        vehicle_id: i64,
        kind: PresenceEventKind,
    ) -> Result<Option<VehiclePresenceEvent>, BusinessError> {
        Ok((self.0.vehicle_id == vehicle_id && self.0.kind == kind).then(|| self.0.clone()))
    }

    async fn latest_of_kind_before(
        &self,
        vehicle_id: i64,
        kind: PresenceEventKind,
        before: NaiveDateTime,
    ) -> Result<Option<VehiclePresenceEvent>, BusinessError> {
        Ok(
            (self.0.vehicle_id == vehicle_id && self.0.kind == kind && self.0.occurred_at < before)
                .then(|| self.0.clone()),
        )
    }
}

#[async_trait]
impl VehiclePresenceStore for QueuePresence {
    async fn get_latest(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<VehiclePresenceEvent>, BusinessError> {
        Ok((self.0.vehicle_id == vehicle_id).then(|| self.0.clone()))
    }

    async fn get_latest_of_kind(
        &self,
        vehicle_id: i64,
        kind: PresenceEventKind,
    ) -> Result<Option<VehiclePresenceEvent>, BusinessError> {
        self.latest_of_kind(vehicle_id, kind).await
    }

    async fn save(&self, _event: VehiclePresenceEvent) -> Result<(), BusinessError> {
        Ok(())
    }
}

struct NoCalls;

#[async_trait]
impl GarageCallStore for NoCalls {
    async fn save(&self, call: GarageCall) -> Result<GarageCall, BusinessError> {
        Ok(call)
    }

    async fn get_latest_uncancelled(
        &self,
        _vehicle_id: i64,
    ) -> Result<Option<GarageCall>, BusinessError> {
        Ok(None)
    }
}

struct NoVehicleLookup;

#[async_trait]
impl VehicleLookupStore for NoVehicleLookup {
    async fn get_by_id(&self, _id: i64) -> Result<Option<Vehicle>, BusinessError> {
        Ok(None)
    }
}

struct UnavailableGauge;

#[async_trait]
impl FuelGaugeReader for UnavailableGauge {
    async fn read_gauge(&self, _vehicle_id: i64) -> Result<TankGauge, BusinessError> {
        Ok(TankGauge::Unavailable(GaugeUnavailable::NoTankRegistered))
    }
}

fn attendance(id: i64, vehicle_id: i64, priority: Option<i32>) -> GarageAttendance {
    let now = Utc::now().naive_utc();
    GarageAttendance {
        id: Some(id),
        uuid: Some(format!("attendance-{id}")),
        tenant_id: Some(42),
        vehicle_id,
        attendance_date: now.date(),
        checked_in_at: now,
        status: GarageAttendanceStatus::Finished,
        manual_priority: priority,
        released_at: Some(now),
        automatic_departure_justification: None,
        origin: GarageAttendanceOrigin::Manual,
        active_marker: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn vehicle(id: i64, prefix: &str) -> Vehicle {
    Vehicle {
        id: Some(id),
        uuid: Some(format!("vehicle-{id}")),
        tenant_id: Some(42),
        plate: format!("PLATE{id}"),
        model: "Truck".into(),
        status: business::domain::enums::VehicleStatus::Active,
        tracker_device_id: None,
        prefix: Some(prefix.into()),
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

fn model(id: i64, name: &str, group: GarageServiceGroup, order: i32) -> GarageServiceModel {
    GarageServiceModel {
        id: Some(id),
        uuid: Some(format!("service-model-{id}")),
        tenant_id: Some(42),
        name: name.into(),
        name_key: name.to_lowercase(),
        display_order: order,
        active: true,
        service_group: group,
        required_for_departure: true,
        governed_by_tank: false,
        applicability: GarageServiceApplicability::All,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn service(id: i64, model_id: i64, state: GarageServiceState, at: NaiveDateTime) -> GarageService {
    GarageService {
        id: Some(id),
        uuid: Some(format!("service-{id}")),
        tenant_id: Some(42),
        attendance_id: 1,
        service_model_id: model_id,
        name_key: format!("service-{model_id}"),
        state,
        performed_at: (state == GarageServiceState::Performed).then_some(at),
        marked_at: Some(at),
        forced_pending_at: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn use_case() -> GarageQueueUseCase {
    let now = Utc::now().naive_utc();
    let models = vec![
        model(2, "Internal", GarageServiceGroup::Internal, 1),
        model(1, "External", GarageServiceGroup::External, 2),
    ];
    let store = QueueStore {
        attendances: vec![attendance(1, 10, Some(1)), attendance(2, 11, None)],
        vehicles: vec![vehicle(10, "A-10")],
        services: vec![
            service(1, 1, GarageServiceState::Performed, now),
            service(2, 2, GarageServiceState::Pending, now),
        ],
        models: models.clone(),
    };
    let departure = VehiclePresenceEvent {
        id: Some(1),
        uuid: Some("departure-10".into()),
        tenant_id: Some(42),
        vehicle_id: 10,
        kind: PresenceEventKind::Departure,
        occurred_at: now,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };
    let schedule = EffectiveScheduleUseCase::new(EmptyScheduleStore);
    let validity = GarageValidityUseCase::new(
        QueuePresence(departure.clone()),
        EmptyTripStore,
        QueueModels(models),
        EmptyRuleStore,
        UnavailableGauge,
    );
    let calls = GarageCallUseCase::new(NoCalls, QueuePresence(departure), NoVehicleLookup);
    GarageQueueUseCase::new(store, schedule, validity, calls)
}

#[tokio::test]
async fn queue_surfaces_share_order_and_matrix_service_state() {
    let use_case = use_case();

    let queue = use_case.queue().await.unwrap();
    assert_eq!(queue.len(), 1, "entries without a vehicle are skipped");
    assert_eq!(queue[0].prefix, "A-10");
    assert!(queue[0].outstanding);
    assert!(
        queue[0].required_pending.is_empty(),
        "a closed triage does not block departure"
    );

    let monitor = use_case.monitor().await.unwrap();
    assert_eq!(monitor.len(), 1);
    assert_eq!(monitor[0].vehicle_uuid, "vehicle-10");

    let (cards, hidden) = use_case.cards().await.unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(hidden, 0);
    assert_eq!(cards[0].tank_percent, None);
    assert!(!cards[0].needs_refuel);

    let matrix = use_case.matrix().await.unwrap();
    assert_eq!(
        matrix
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        vec!["External", "Internal"]
    );
    assert_eq!(matrix.rows.len(), 1);
    assert_eq!(matrix.rows[0].cells.len(), 2);
    assert_eq!(
        matrix.rows[0].cells[0].as_ref().unwrap().effective_state,
        GarageServiceState::Performed
    );
    assert_eq!(
        matrix.rows[0].cells[1].as_ref().unwrap().effective_state,
        GarageServiceState::Pending
    );

    assert!(use_case.call_list().await.unwrap().is_empty());
}
