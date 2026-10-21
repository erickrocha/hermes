use business::domain::business_error::BusinessError;
use business::domain::enums::{
    FuelEntryOrigin, GarageAttendanceOrigin, GarageAttendanceStatus, GarageServiceGroup,
    GarageServiceState,
};
use business::domain::fuel_entry::FuelEntry;
use business::domain::garage_attendance::GarageAttendance;
use business::domain::garage_service::GarageService;
use business::domain::garage_service_log::GarageServiceLog;
use business::domain::garage_service_model::{GarageServiceApplicability, GarageServiceModel};
use business::domain::tenant_rule_setting::RuleSettings;
use business::gateway::garage_fuelling_mark_gateway::GarageFuellingMarkStore;
use business::use_cases::garage_fuelling_mark_use_case::GarageFuellingMarkUseCase;
use chrono::{NaiveDate, NaiveDateTime};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

struct MemoryFuellingStore {
    later_reads: Arc<AtomicUsize>,
    fuel: Option<FuelEntry>,
    services: Vec<GarageService>,
    model: Option<GarageServiceModel>,
    saved_service: Arc<Mutex<Option<GarageService>>>,
    saved_log: Arc<Mutex<Option<GarageServiceLog>>>,
}

#[async_trait]
impl GarageFuellingMarkStore for MemoryFuellingStore {
    async fn active_attendances(&self) -> Result<Vec<GarageAttendance>, BusinessError> {
        Ok(vec![attendance()])
    }

    async fn active_attendance(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<GarageAttendance>, BusinessError> {
        Ok((vehicle_id == 10).then(attendance))
    }

    async fn latest_full_tank_fuel(
        &self,
        _vehicle_id: i64,
    ) -> Result<Option<FuelEntry>, BusinessError> {
        Ok(self.fuel.clone())
    }

    async fn services(&self, _attendance_id: i64) -> Result<Vec<GarageService>, BusinessError> {
        self.later_reads.fetch_add(1, Ordering::Relaxed);
        Ok(self.services.clone())
    }

    async fn service_model(
        &self,
        model_id: i64,
    ) -> Result<Option<GarageServiceModel>, BusinessError> {
        self.later_reads.fetch_add(1, Ordering::Relaxed);
        Ok(self
            .model
            .as_ref()
            .filter(|model| model.id == Some(model_id))
            .cloned())
    }

    async fn rules(&self, _tenant_id: Option<i64>) -> Result<RuleSettings, BusinessError> {
        self.later_reads.fetch_add(1, Ordering::Relaxed);
        Ok(RuleSettings::default())
    }

    async fn last_arrival(&self, _vehicle_id: i64) -> Result<Option<NaiveDateTime>, BusinessError> {
        self.later_reads.fetch_add(1, Ordering::Relaxed);
        Ok(None)
    }

    async fn save_service(&self, service: GarageService) -> Result<GarageService, BusinessError> {
        self.later_reads.fetch_add(1, Ordering::Relaxed);
        *self
            .saved_service
            .lock()
            .map_err(|_| BusinessError::new("service capture poisoned".into()))? =
            Some(service.clone());
        Ok(service)
    }

    async fn save_log(&self, log: GarageServiceLog) -> Result<(), BusinessError> {
        self.later_reads.fetch_add(1, Ordering::Relaxed);
        *self
            .saved_log
            .lock()
            .map_err(|_| BusinessError::new("log capture poisoned".into()))? = Some(log);
        Ok(())
    }
}

fn attendance() -> GarageAttendance {
    GarageAttendance {
        id: Some(4),
        uuid: Some("attendance-4".into()),
        tenant_id: Some(42),
        vehicle_id: 10,
        attendance_date: NaiveDate::from_ymd_opt(2026, 10, 1).expect("valid date"),
        checked_in_at: NaiveDateTime::parse_from_str("2026-10-01 08:00:00", "%Y-%m-%d %H:%M:%S")
            .expect("valid timestamp"),
        status: GarageAttendanceStatus::Open,
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

#[tokio::test]
async fn no_eligible_full_tank_fuelling_skips_the_remaining_store_calls() {
    let later_reads = Arc::new(AtomicUsize::new(0));
    let use_case = GarageFuellingMarkUseCase::new(MemoryFuellingStore {
        later_reads: Arc::clone(&later_reads),
        fuel: None,
        services: Vec::new(),
        model: None,
        saved_service: Arc::default(),
        saved_log: Arc::default(),
    });

    assert!(!use_case.apply(10).await.expect("mark check succeeds"));
    assert_eq!(later_reads.load(Ordering::Relaxed), 0);
    assert_eq!(
        use_case
            .apply_all_active()
            .await
            .expect("batch check succeeds"),
        0
    );
    assert_eq!(later_reads.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn a_fresh_full_tank_fuelling_marks_the_tank_governed_service_and_logs_its_source() {
    let later_reads = Arc::new(AtomicUsize::new(0));
    let saved_service = Arc::new(Mutex::new(None));
    let saved_log = Arc::new(Mutex::new(None));
    let now = chrono::Utc::now().naive_utc();
    let fuelled_at = now - chrono::Duration::minutes(5);
    let store = MemoryFuellingStore {
        later_reads: Arc::clone(&later_reads),
        fuel: Some(FuelEntry {
            id: Some(99),
            uuid: Some("fuel-99".into()),
            tenant_id: Some(42),
            vehicle_id: 10,
            recorded_at: fuelled_at,
            volume_liters: 100.0,
            value_cents: 50_000,
            odometer_km: Some(10_000.0),
            station: Some("Station".into()),
            full_tank: true,
            origin: FuelEntryOrigin::Manual,
            provider_transaction_id: None,
            reported_by_user_id: None,
            odometer_override_note: None,
            provider_confirmed_at: None,
            deleted_at: None,
            unified_into_id: None,
            unification_note: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }),
        services: vec![GarageService {
            id: Some(6),
            uuid: Some("service-6".into()),
            tenant_id: Some(42),
            attendance_id: 4,
            service_model_id: 7,
            name_key: "fuelling".into(),
            state: GarageServiceState::Pending,
            performed_at: None,
            marked_at: None,
            forced_pending_at: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }],
        model: Some(GarageServiceModel {
            id: Some(7),
            uuid: Some("model-7".into()),
            tenant_id: Some(42),
            name: "Fuelling".into(),
            name_key: "fuelling".into(),
            display_order: 1,
            active: true,
            service_group: GarageServiceGroup::External,
            required_for_departure: false,
            governed_by_tank: true,
            applicability: GarageServiceApplicability::All,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }),
        saved_service: Arc::clone(&saved_service),
        saved_log: Arc::clone(&saved_log),
    };
    let use_case = GarageFuellingMarkUseCase::new(store);

    assert!(use_case.apply(10).await.unwrap());
    let marked = saved_service.lock().unwrap().clone().unwrap();
    assert_eq!(marked.state, GarageServiceState::Performed);
    assert_eq!(marked.performed_at, Some(fuelled_at));
    assert_eq!(marked.marked_at, None);
    assert_eq!(marked.forced_pending_at, None);
    let log = saved_log.lock().unwrap().clone().unwrap();
    assert_eq!(log.new_state, GarageServiceState::Performed);
    assert_eq!(log.origin, "System: fuelling 99");
    assert_eq!(later_reads.load(Ordering::Relaxed), 6);
}
