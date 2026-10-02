use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::GarageAttendanceStatus;
use crate::domain::garage_attendance::{GarageAttendance, GarageAttendanceEntityMapper};
use crate::gateway::garage_attendance_gateway::GarageAttendanceGateway;
use crate::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway;
use crate::use_cases::garage_readiness::{Rollover, operating_date, rollover};
use crate::use_cases::vehicle_presence_use_case::VehiclePresenceUseCase;
use sea_orm::DbErr;

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
    attendances: GarageAttendanceGateway,
    presence: VehiclePresenceUseCase,
    rules: TenantRuleSettingGateway,
}

impl GarageRolloverUseCase {
    pub fn new(attendances: GarageAttendanceGateway, presence: VehiclePresenceUseCase, rules: TenantRuleSettingGateway) -> Self {
        Self { attendances, presence, rules }
    }

    pub async fn run(&self, tenant_id: Option<i64>) -> Result<RolloverOutcome, BusinessError> {
        let now = chrono::Utc::now().naive_utc();
        let today = operating_date(now, &self.rules.settings_for(tenant_id).await.map_err(database_error)?);
        let mut outcome = RolloverOutcome::default();
        for model in self.attendances.find_all_active().await.map_err(database_error)? {
            let attendance = GarageAttendanceEntityMapper::from_model(model);
            let away = self.presence.snapshot(attendance.vehicle_id).await?.away;
            match rollover(attendance.attendance_date, today, away) {
                Rollover::Keep => {}
                Rollover::Redate => {
                    self.save(GarageAttendance { attendance_date: today, manual_priority: None, ..attendance }).await?;
                    outcome.redated += 1;
                }
                Rollover::Close => {
                    self.save(GarageAttendance {
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

    async fn save(&self, attendance: GarageAttendance) -> Result<(), BusinessError> {
        self.attendances.persist(attendance).await.map(|_| ()).map_err(database_error)
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[GarageRolloverUseCase] {}", msg);
    BusinessError::new(msg)
}
