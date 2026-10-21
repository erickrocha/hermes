use business::domain::business_error::BusinessError;
use business::domain::enums::{
    GarageAttendanceOrigin, GarageAttendanceStatus, GarageServiceGroup, GarageServiceState,
    VehicleStatus,
};
use business::domain::garage_attendance::GarageAttendance;
use business::domain::garage_service::GarageService;
use business::domain::garage_service_log::GarageServiceLog;
use business::domain::garage_service_model::{GarageServiceApplicability, GarageServiceModel};
use business::domain::vehicle::Vehicle;
use business::gateway::garage_attendance_gateway::GarageAttendanceGateway;
use business::gateway::garage_attendance_workflow_gateway::GarageAttendanceDatabaseGateway;
use business::gateway::garage_attendance_workflow_gateway::GarageAttendanceStore;
use business::gateway::garage_service_gateway::GarageServiceGateway;
use business::gateway::garage_service_log_gateway::GarageServiceLogGateway;
use business::gateway::garage_service_model_gateway::GarageServiceModelGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::use_cases::garage_attendance_use_case::{
    ALREADY_ACTIVE, ATTENDANCE_CLOSED, ATTENDANCE_NOT_FOUND, GarageAttendanceUseCase, NO_SERVICES,
    SERVICE_NOT_FOUND, VEHICLE_NOT_FOUND,
};
use chrono::NaiveDate;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{DatabaseBackend, MockDatabase};
use std::sync::Mutex;

#[derive(Default)]
struct MemoryAttendanceStore {
    vehicle: Option<Vehicle>,
    active: Option<GarageAttendance>,
    latest: Option<GarageAttendance>,
    catalogue: Vec<GarageServiceModel>,
    attendance: Option<GarageAttendance>,
    services: Vec<GarageService>,
    service_for_uuid: Option<GarageService>,
    logs: Vec<GarageServiceLog>,
}

#[derive(Clone)]
struct TestStore {
    state: std::sync::Arc<Mutex<MemoryAttendanceStore>>,
}

#[async_trait]
impl GarageAttendanceStore for TestStore {
    async fn find_vehicle(&self, _id: i64) -> Result<Option<Vehicle>, BusinessError> {
        Ok(self.state.lock().unwrap().vehicle.clone())
    }

    async fn find_active_attendance(
        &self,
        _vehicle_id: i64,
    ) -> Result<Option<GarageAttendance>, BusinessError> {
        Ok(self.state.lock().unwrap().active.clone())
    }

    async fn find_latest_attendance(
        &self,
        _vehicle_id: i64,
    ) -> Result<Option<GarageAttendance>, BusinessError> {
        Ok(self.state.lock().unwrap().latest.clone())
    }

    async fn active_service_models(&self) -> Result<Vec<GarageServiceModel>, BusinessError> {
        Ok(self.state.lock().unwrap().catalogue.clone())
    }

    async fn save_attendance(
        &self,
        mut attendance: GarageAttendance,
    ) -> Result<GarageAttendance, BusinessError> {
        let mut store = self.state.lock().unwrap();
        attendance.id.get_or_insert(55);
        attendance
            .uuid
            .get_or_insert_with(|| "attendance-55".into());
        store.attendance = Some(attendance.clone());
        store.active =
            (attendance.status == GarageAttendanceStatus::Open).then(|| attendance.clone());
        Ok(attendance)
    }

    async fn find_attendance_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<GarageAttendance>, BusinessError> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .attendance
            .clone()
            .filter(|attendance| attendance.uuid.as_deref() == Some(uuid.as_str())))
    }

    async fn list_services(&self, attendance_id: i64) -> Result<Vec<GarageService>, BusinessError> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .services
            .iter()
            .filter(|service| service.attendance_id == attendance_id)
            .cloned()
            .collect())
    }

    async fn find_service_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<GarageService>, BusinessError> {
        let store = self.state.lock().unwrap();
        Ok(store
            .service_for_uuid
            .clone()
            .filter(|service| service.uuid.as_deref() == Some(uuid.as_str()))
            .or_else(|| {
                store
                    .services
                    .iter()
                    .find(|service| service.uuid.as_deref() == Some(uuid.as_str()))
                    .cloned()
            }))
    }

    async fn save_service(
        &self,
        mut service: GarageService,
    ) -> Result<GarageService, BusinessError> {
        let mut store = self.state.lock().unwrap();
        service.id.get_or_insert(77);
        service.uuid.get_or_insert_with(|| "service-77".into());
        store.service_for_uuid = Some(service.clone());
        store.services.retain(|existing| existing.id != service.id);
        store.services.push(service.clone());
        Ok(service)
    }

    async fn save_service_log(&self, log: GarageServiceLog) -> Result<(), BusinessError> {
        self.state.lock().unwrap().logs.push(log);
        Ok(())
    }
}

fn vehicle() -> Vehicle {
    Vehicle {
        id: Some(10),
        uuid: Some("vehicle-10".into()),
        tenant_id: Some(42),
        plate: "ABC123".into(),
        model: "Bus".into(),
        status: VehicleStatus::Active,
        tracker_device_id: None,
        prefix: None,
        vehicle_type: Some("Bus".into()),
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

fn attendance(status: GarageAttendanceStatus) -> GarageAttendance {
    let now = NaiveDate::from_ymd_opt(2026, 10, 21)
        .unwrap()
        .and_hms_opt(10, 0, 0)
        .unwrap();
    GarageAttendance {
        id: Some(55),
        uuid: Some("attendance-55".into()),
        tenant_id: Some(42),
        vehicle_id: 10,
        attendance_date: now.date(),
        checked_in_at: now,
        status,
        manual_priority: Some(3),
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
        id: Some(77),
        uuid: Some("service-77".into()),
        tenant_id: Some(42),
        attendance_id: 55,
        service_model_id: 8,
        name_key: "wash".into(),
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

fn catalogue() -> GarageServiceModel {
    GarageServiceModel {
        id: Some(8),
        uuid: Some("model-8".into()),
        tenant_id: Some(42),
        name: "Wash".into(),
        name_key: "wash".into(),
        display_order: 1,
        active: true,
        service_group: GarageServiceGroup::External,
        required_for_departure: false,
        governed_by_tank: false,
        applicability: GarageServiceApplicability::All,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn memory_use_case(
    store: MemoryAttendanceStore,
) -> (
    GarageAttendanceUseCase,
    std::sync::Arc<Mutex<MemoryAttendanceStore>>,
) {
    let store = std::sync::Arc::new(Mutex::new(store));
    (
        GarageAttendanceUseCase::new(TestStore {
            state: store.clone(),
        }),
        store,
    )
}

#[tokio::test]
async fn opening_a_triage_for_a_missing_vehicle_stops_after_the_vehicle_lookup() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([Vec::<entity::vehicle_entity::Model>::new()])
        .into_connection();
    let use_case = GarageAttendanceUseCase::new(GarageAttendanceDatabaseGateway::new(
        GarageAttendanceGateway::new(db.clone()),
        GarageServiceGateway::new(db.clone()),
        GarageServiceLogGateway::new(db.clone()),
        GarageServiceModelGateway::new(db.clone()),
        VehicleGateway::new(db.clone()),
    ));

    let error = use_case
        .open(
            Some(42),
            10,
            None,
            business::domain::enums::GarageAttendanceOrigin::Manual,
        )
        .await
        .expect_err("a missing vehicle cannot enter the garage");

    assert_eq!(error.message, VEHICLE_NOT_FOUND);
    let queries = db.into_transaction_log();
    assert_eq!(queries.len(), 1);
    assert!(!format!("{queries:?}").contains("INSERT"));
}

#[tokio::test]
async fn opening_a_triage_creates_one_pending_service_for_each_applicable_catalogue_entry() {
    let (use_case, store) = memory_use_case(MemoryAttendanceStore {
        vehicle: Some(vehicle()),
        catalogue: vec![catalogue()],
        ..Default::default()
    });

    let (opened, services) = use_case
        .open(Some(42), 10, Some(4), GarageAttendanceOrigin::Manual)
        .await
        .expect("vehicle with an applicable service can be checked in");

    assert_eq!(opened.id, Some(55));
    assert_eq!(opened.manual_priority, Some(4));
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].state, GarageServiceState::Pending);
    assert_eq!(store.lock().unwrap().services.len(), 1);
}

#[tokio::test]
async fn opening_a_triage_rejects_an_existing_active_attendance_or_empty_catalogue() {
    let (use_case, _) = memory_use_case(MemoryAttendanceStore {
        vehicle: Some(vehicle()),
        active: Some(attendance(GarageAttendanceStatus::Open)),
        catalogue: vec![catalogue()],
        ..Default::default()
    });
    let error = use_case
        .open(Some(42), 10, None, GarageAttendanceOrigin::Manual)
        .await
        .expect_err("a vehicle cannot have two active triages");
    assert_eq!(error.message, ALREADY_ACTIVE);

    let (use_case, _) = memory_use_case(MemoryAttendanceStore {
        vehicle: Some(vehicle()),
        ..Default::default()
    });
    let error = use_case
        .open(Some(42), 10, None, GarageAttendanceOrigin::Manual)
        .await
        .expect_err("a triage needs at least one active service");
    assert_eq!(error.message, NO_SERVICES);
}

#[tokio::test]
async fn arrival_opens_a_triage_and_carries_service_state_when_the_vehicle_did_not_make_a_trip() {
    let mut completed = service(GarageServiceState::Performed);
    completed.performed_at = Some(
        NaiveDate::from_ymd_opt(2026, 10, 20)
            .unwrap()
            .and_hms_opt(9, 0, 0)
            .unwrap(),
    );
    let (use_case, _) = memory_use_case(MemoryAttendanceStore {
        vehicle: Some(vehicle()),
        latest: Some(attendance(GarageAttendanceStatus::Finished)),
        catalogue: vec![catalogue()],
        services: vec![completed],
        ..Default::default()
    });

    let opened = use_case
        .open_on_arrival(Some(42), 10, None, 60)
        .await
        .expect("arrival triage can be opened")
        .expect("a matching service is available");

    assert_eq!(opened.0.origin, GarageAttendanceOrigin::ArrivalAtBase);
    assert_eq!(opened.1[0].state, GarageServiceState::Performed);
    assert!(opened.1[0].performed_at.is_some());
}

#[tokio::test]
async fn arrival_is_idempotent_for_an_active_triage_and_a_vehicle_returning_from_a_trip_starts_clean()
 {
    let (use_case, _) = memory_use_case(MemoryAttendanceStore {
        active: Some(attendance(GarageAttendanceStatus::Open)),
        ..Default::default()
    });
    assert!(
        use_case
            .open_on_arrival(Some(42), 10, None, 60)
            .await
            .expect("already checked in is not an error")
            .is_none()
    );

    let (use_case, _) = memory_use_case(MemoryAttendanceStore {
        vehicle: Some(vehicle()),
        latest: Some(attendance(GarageAttendanceStatus::Finished)),
        catalogue: vec![catalogue()],
        services: vec![service(GarageServiceState::Performed)],
        ..Default::default()
    });
    let departed = NaiveDate::from_ymd_opt(2026, 10, 21)
        .unwrap()
        .and_hms_opt(8, 0, 0)
        .unwrap();
    let arrived = departed + chrono::Duration::minutes(90);
    let opened = use_case
        .open_on_arrival(Some(42), 10, Some((departed, arrived)), 60)
        .await
        .expect("return opens a fresh triage")
        .expect("catalogue contains applicable services");
    assert_eq!(opened.1[0].state, GarageServiceState::Pending);
}

#[tokio::test]
async fn arrival_with_no_applicable_services_is_a_noop() {
    let mut model = catalogue();
    model.applicability = GarageServiceApplicability::Vehicles(vec!["another-vehicle".into()]);
    let (use_case, _) = memory_use_case(MemoryAttendanceStore {
        vehicle: Some(vehicle()),
        catalogue: vec![model],
        ..Default::default()
    });
    assert!(
        use_case
            .open_on_arrival(Some(42), 10, None, 60)
            .await
            .expect("an empty applicable catalogue is not an arrival failure")
            .is_none()
    );
}

#[tokio::test]
async fn marking_a_service_updates_its_state_and_writes_an_audit_log() {
    let (use_case, store) = memory_use_case(MemoryAttendanceStore {
        attendance: Some(attendance(GarageAttendanceStatus::Open)),
        services: vec![service(GarageServiceState::Pending)],
        ..Default::default()
    });
    let marked = use_case
        .mark_service(
            "attendance-55".into(),
            "service-77".into(),
            GarageServiceState::Performed,
            Some(12),
        )
        .await
        .expect("service on open triage can be marked");
    assert_eq!(marked.state, GarageServiceState::Performed);
    assert!(marked.performed_at.is_some());
    assert_eq!(store.lock().unwrap().logs.len(), 1);
}

#[tokio::test]
async fn marking_rejects_unknown_closed_or_unrelated_services() {
    let (use_case, _) = memory_use_case(MemoryAttendanceStore::default());
    assert_eq!(
        use_case
            .mark_service(
                "missing".into(),
                "service-77".into(),
                GarageServiceState::Pending,
                None,
            )
            .await
            .expect_err("unknown triage is rejected")
            .message,
        ATTENDANCE_NOT_FOUND
    );

    let (use_case, _) = memory_use_case(MemoryAttendanceStore {
        attendance: Some(attendance(GarageAttendanceStatus::Finished)),
        ..Default::default()
    });
    assert_eq!(
        use_case
            .mark_service(
                "attendance-55".into(),
                "service-77".into(),
                GarageServiceState::Pending,
                None,
            )
            .await
            .expect_err("closed triage cannot be changed")
            .message,
        ATTENDANCE_CLOSED
    );

    let mut unrelated = service(GarageServiceState::Pending);
    unrelated.attendance_id = 99;
    let (use_case, _) = memory_use_case(MemoryAttendanceStore {
        attendance: Some(attendance(GarageAttendanceStatus::Open)),
        service_for_uuid: Some(unrelated),
        ..Default::default()
    });
    assert_eq!(
        use_case
            .mark_service(
                "attendance-55".into(),
                "service-77".into(),
                GarageServiceState::Pending,
                None,
            )
            .await
            .expect_err("service must belong to this triage")
            .message,
        SERVICE_NOT_FOUND
    );
}

#[tokio::test]
async fn checkout_closes_an_open_triage_and_active_lookup_returns_its_services() {
    let (use_case, store) = memory_use_case(MemoryAttendanceStore {
        active: Some(attendance(GarageAttendanceStatus::Open)),
        attendance: Some(attendance(GarageAttendanceStatus::Open)),
        services: vec![service(GarageServiceState::Pending)],
        ..Default::default()
    });
    let active = use_case
        .find_active(10)
        .await
        .expect("active attendance is retrieved")
        .expect("attendance exists");
    assert_eq!(active.1.len(), 1);

    let closed = use_case
        .checkout("attendance-55".into())
        .await
        .expect("open triage can be checked out");
    assert_eq!(closed.status, GarageAttendanceStatus::Finished);
    assert_eq!(closed.manual_priority, None);
    assert!(closed.released_at.is_some());
    assert!(store.lock().unwrap().active.is_none());
}

#[tokio::test]
async fn lookup_and_checkout_report_missing_or_closed_triages() {
    let (use_case, _) = memory_use_case(MemoryAttendanceStore::default());
    assert_eq!(
        use_case
            .find_by_uuid("missing".into())
            .await
            .expect_err("unknown attendance is rejected")
            .message,
        ATTENDANCE_NOT_FOUND
    );

    let (use_case, _) = memory_use_case(MemoryAttendanceStore {
        attendance: Some(attendance(GarageAttendanceStatus::Finished)),
        ..Default::default()
    });
    assert_eq!(
        use_case
            .checkout("attendance-55".into())
            .await
            .expect_err("closed triage cannot be checked out twice")
            .message,
        ATTENDANCE_CLOSED
    );

    let (use_case, _) = memory_use_case(MemoryAttendanceStore::default());
    assert!(
        use_case
            .find_active(10)
            .await
            .expect("no active triage is not an error")
            .is_none()
    );
}
