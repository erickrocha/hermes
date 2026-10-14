use business::domain::business_error::BusinessError;
use business::domain::enums::{GarageAttendanceOrigin, GarageAttendanceStatus};
use business::domain::garage_attendance::GarageAttendance;
use business::domain::tenant_rule_setting::RuleSettings;
use business::gateway::garage_rollover_gateway::GarageRolloverStore;
use business::use_cases::garage_rollover_use_case::GarageRolloverUseCase;
use business::use_cases::vehicle_presence_use_case::{PresenceSnapshot, VehiclePresenceReader};
use chrono::{NaiveDate, NaiveDateTime};
use sea_orm::prelude::async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryGarageRolloverStore {
    attendances: Mutex<Vec<GarageAttendance>>,
}

#[async_trait]
impl GarageRolloverStore for MemoryGarageRolloverStore {
    async fn settings_for(&self, _tenant_id: Option<i64>) -> Result<RuleSettings, BusinessError> {
        Ok(RuleSettings::default())
    }

    async fn find_all_active(&self) -> Result<Vec<GarageAttendance>, BusinessError> {
        let attendances = self
            .attendances
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(attendances
            .iter()
            .filter(|attendance| attendance.active_marker.is_some())
            .cloned()
            .collect())
    }

    async fn save(&self, attendance: GarageAttendance) -> Result<(), BusinessError> {
        let mut attendances = self
            .attendances
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let existing = attendances
            .iter_mut()
            .find(|existing| existing.id == attendance.id);
        match existing {
            Some(existing) => *existing = attendance,
            None => attendances.push(attendance),
        }
        Ok(())
    }
}

struct MemoryPresenceReader {
    away_by_vehicle: HashMap<i64, bool>,
}

#[async_trait]
impl VehiclePresenceReader for MemoryPresenceReader {
    async fn snapshot(&self, vehicle_id: i64) -> Result<PresenceSnapshot, BusinessError> {
        Ok(PresenceSnapshot {
            last_arrival_at: None,
            last_departure_at: None,
            away: self
                .away_by_vehicle
                .get(&vehicle_id)
                .copied()
                .unwrap_or(false),
        })
    }
}

fn old_attendance(id: i64, vehicle_id: i64) -> GarageAttendance {
    GarageAttendance {
        id: Some(id),
        uuid: Some(format!("attendance-{id}")),
        tenant_id: Some(7),
        vehicle_id,
        attendance_date: NaiveDate::from_ymd_opt(2000, 1, 1).expect("valid fixed date"),
        checked_in_at: NaiveDateTime::parse_from_str("2000-01-01 08:00:00", "%Y-%m-%d %H:%M:%S")
            .expect("valid fixed timestamp"),
        status: GarageAttendanceStatus::default(),
        manual_priority: Some(3),
        released_at: None,
        automatic_departure_justification: None,
        origin: GarageAttendanceOrigin::default(),
        active_marker: Some(1),
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn rollover_redates_present_vehicles_closes_away_vehicles_and_is_idempotent() {
    let store = MemoryGarageRolloverStore {
        attendances: Mutex::new(vec![old_attendance(1, 11), old_attendance(2, 22)]),
    };
    let use_case = GarageRolloverUseCase::new(
        store,
        MemoryPresenceReader {
            away_by_vehicle: HashMap::from([(22, true)]),
        },
    );

    let first = use_case
        .run(Some(7))
        .await
        .expect("first rollover succeeds");
    assert_eq!(first.redated, 1);
    assert_eq!(first.closed, 1);

    let second = use_case
        .run(Some(7))
        .await
        .expect("second rollover succeeds");
    assert_eq!(second, Default::default());
}
