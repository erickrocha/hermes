use business::domain::business_error::BusinessError;
use business::domain::enums::{GarageAttendanceOrigin, GarageAttendanceStatus};
use business::domain::fuel_entry::FuelEntry;
use business::domain::garage_attendance::GarageAttendance;
use business::domain::garage_service::GarageService;
use business::domain::garage_service_log::GarageServiceLog;
use business::domain::garage_service_model::GarageServiceModel;
use business::domain::tenant_rule_setting::RuleSettings;
use business::gateway::garage_fuelling_mark_gateway::GarageFuellingMarkStore;
use business::use_cases::garage_fuelling_mark_use_case::GarageFuellingMarkUseCase;
use chrono::{NaiveDate, NaiveDateTime};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct MemoryFuellingStore {
    later_reads: Arc<AtomicUsize>,
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
        Ok(None)
    }

    async fn services(&self, _attendance_id: i64) -> Result<Vec<GarageService>, BusinessError> {
        self.later_reads.fetch_add(1, Ordering::Relaxed);
        Ok(Vec::new())
    }

    async fn service_model(
        &self,
        _model_id: i64,
    ) -> Result<Option<GarageServiceModel>, BusinessError> {
        self.later_reads.fetch_add(1, Ordering::Relaxed);
        Ok(None)
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
        Ok(service)
    }

    async fn save_log(&self, _log: GarageServiceLog) -> Result<(), BusinessError> {
        self.later_reads.fetch_add(1, Ordering::Relaxed);
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
