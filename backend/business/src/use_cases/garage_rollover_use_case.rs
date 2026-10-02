use crate::domain::business_error::BusinessError;
use crate::domain::enums::GarageAttendanceStatus;
use crate::domain::garage_attendance::GarageAttendance;
use crate::gateway::garage_rollover_gateway::GarageRolloverStore;
use crate::use_cases::garage_readiness::{Rollover, operating_date, rollover};
use crate::use_cases::vehicle_presence_use_case::VehiclePresenceReader;

/// What one pass did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RolloverOutcome {
    pub closed: usize,
    pub redated: usize,
}

/// `EPIC-GA-06-S03` (`HRMS-970`, `TRM-419…422`, `426`): the day boundary over one
/// tenant's open triages. A triage dated before today whose vehicle is still at
/// base is re-dated to today, its priority zeroed (`TRM-421`/`422`/`426`) -- its
/// service records are not touched, so a stale status never wins a merge
/// (`TRM-422`); one whose vehicle has left is closed as `LeftForOperation`
/// (`TRM-419`); today's stays open whatever the vehicle did (`TRM-420`).
///
/// Idempotent: a second pass over the same day finds nothing to do, so two
/// instances running at once are harmless. The scope is the caller's -- the
/// scheduler runs it once per tenant under `run_for_tenant`.
pub struct GarageRolloverUseCase {
    store: Box<dyn GarageRolloverStore>,
    presence: Box<dyn VehiclePresenceReader>,
}

impl GarageRolloverUseCase {
    pub fn new<S, P>(store: S, presence: P) -> Self
    where
        S: GarageRolloverStore + 'static,
        P: VehiclePresenceReader + 'static,
    {
        Self {
            store: Box::new(store),
            presence: Box::new(presence),
        }
    }

    pub async fn run(&self, tenant_id: Option<i64>) -> Result<RolloverOutcome, BusinessError> {
        let now = chrono::Utc::now().naive_utc();
        let rules = self.store.settings_for(tenant_id).await?;
        let today = operating_date(now, &rules);
        let mut outcome = RolloverOutcome::default();
        for attendance in self.store.find_all_active().await? {
            let away = self.presence.snapshot(attendance.vehicle_id).await?.away;
            match rollover(attendance.attendance_date, today, away) {
                Rollover::Keep => {}
                Rollover::Redate => {
                    self.store
                        .save(GarageAttendance {
                            attendance_date: today,
                            manual_priority: None,
                            ..attendance
                        })
                        .await?;
                    outcome.redated += 1;
                }
                Rollover::Close => {
                    self.store
                        .save(GarageAttendance {
                            status: GarageAttendanceStatus::LeftForOperation,
                            released_at: Some(now),
                            active_marker: None,
                            manual_priority: None,
                            ..attendance
                        })
                        .await?;
                    outcome.closed += 1;
                }
            }
        }
        Ok(outcome)
    }
}
