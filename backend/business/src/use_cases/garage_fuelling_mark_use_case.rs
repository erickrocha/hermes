use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::GarageServiceState;
use crate::domain::garage_attendance::GarageAttendanceEntityMapper;
use crate::domain::garage_service::{GarageService, GarageServiceEntityMapper};
use crate::domain::garage_service_log::GarageServiceLog;
use crate::gateway::fuel_entry_gateway::FuelEntryGateway;
use crate::gateway::garage_attendance_gateway::GarageAttendanceGateway;
use crate::gateway::garage_service_gateway::GarageServiceGateway;
use crate::gateway::garage_service_log_gateway::GarageServiceLogGateway;
use crate::gateway::garage_service_model_gateway::GarageServiceModelGateway;
use crate::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway;
use crate::gateway::vehicle_presence_event_gateway::VehiclePresenceEventGateway;
use crate::use_cases::garage_readiness::should_mark_fuelling;
use sea_orm::DbErr;

/// `EPIC-GA-06-S02` (`HRMS-969`, `TRM-1520…1529`): a provider-reported full tank
/// marks the fuelling service of the vehicle's open triage by itself.
///
/// Idempotent by construction: once marked, the fuelling is no longer newer than
/// the service's confirmation, so running it again changes nothing. That is why
/// it can be called from every place a fuelling or a triage appears -- the
/// provider sync, and a triage opened hours after the fuelling (`TRM-1522`).
pub struct GarageFuellingMarkUseCase {
    attendances: GarageAttendanceGateway,
    services: GarageServiceGateway,
    logs: GarageServiceLogGateway,
    models: GarageServiceModelGateway,
    fuel_entries: FuelEntryGateway,
    events: VehiclePresenceEventGateway,
    rules: TenantRuleSettingGateway,
}

impl GarageFuellingMarkUseCase {
    pub fn new(
        attendances: GarageAttendanceGateway,
        services: GarageServiceGateway,
        logs: GarageServiceLogGateway,
        models: GarageServiceModelGateway,
        fuel_entries: FuelEntryGateway,
        events: VehiclePresenceEventGateway,
        rules: TenantRuleSettingGateway,
    ) -> Self {
        Self { attendances, services, logs, models, fuel_entries, events, rules }
    }

    /// Every open triage of the caller's tenant (`TRM-1528`: only the current
    /// triage is ever touched). Returns how many services were marked.
    pub async fn apply_all_active(&self) -> Result<usize, BusinessError> {
        let mut marked = 0;
        for model in self.attendances.find_all_active().await.map_err(database_error)? {
            if self.apply_to(GarageAttendanceEntityMapper::from_model(model)).await? {
                marked += 1;
            }
        }
        Ok(marked)
    }

    /// One vehicle's open triage. `false` when there is nothing to mark.
    pub async fn apply(&self, vehicle_id: i64) -> Result<bool, BusinessError> {
        match self.attendances.find_active_by_vehicle(vehicle_id).await.map_err(database_error)? {
            Some(model) => self.apply_to(GarageAttendanceEntityMapper::from_model(model)).await,
            None => Ok(false),
        }
    }

    async fn apply_to(&self, attendance: crate::domain::garage_attendance::GarageAttendance) -> Result<bool, BusinessError> {
        let vehicle_id = attendance.vehicle_id;
        let Some(fuelling) = self.fuel_entries.find_latest_provider_full_tank(vehicle_id).await.map_err(database_error)? else {
            return Ok(false);
        };
        let fuelled_at = fuelling.recorded_at.naive_utc();
        let records = GarageServiceEntityMapper::from_models(
            self.services.find_by_attendance(attendance.id.unwrap_or_default()).await.map_err(database_error)?,
        );
        // The fuelling service is the catalogue's tank-governed one, never a name match (`TRM-462`).
        let mut fuelling_service: Option<GarageService> = None;
        for record in records {
            let model = self.models.find_by_id(record.service_model_id).await.map_err(database_error)?;
            if model.is_some_and(|m| m.governed_by_tank) {
                fuelling_service = Some(record);
                break;
            }
        }
        let Some(service) = fuelling_service else { return Ok(false) };

        let rules = self.rules.settings_for(attendance.tenant_id).await.map_err(database_error)?;
        let last_arrival = self
            .events
            .find_latest_of_kind(vehicle_id, "Arrival")
            .await
            .map_err(database_error)?
            .map(|m| m.occurred_at.naive_utc());
        let confirmation = service.performed_at.into_iter().chain(service.marked_at).max();
        let now = chrono::Utc::now().naive_utc();
        if !should_mark_fuelling(fuelled_at, now, confirmation, last_arrival, service.forced_pending_at, &rules) {
            return Ok(false);
        }

        // `TRM-1526`: the mark carries the fuelling's own instant, clears the forcing stamp.
        let saved = self
            .services
            .persist(GarageService {
                state: GarageServiceState::Performed,
                performed_at: Some(fuelled_at),
                marked_at: None,
                forced_pending_at: None,
                ..service
            })
            .await
            .map_err(database_error)?;
        let saved = GarageServiceEntityMapper::from_active_model(saved);
        self.logs
            .persist(GarageServiceLog {
                id: None,
                uuid: None,
                tenant_id: saved.tenant_id,
                attendance_id: saved.attendance_id,
                vehicle_id,
                service_model_id: saved.service_model_id,
                name_key: saved.name_key.clone(),
                new_state: GarageServiceState::Performed,
                acted_by_user_id: None,
                acted_at: now,
                // The evidence: which fuelling made the mark.
                origin: format!("System: fuelling {}", fuelling.id),
                created_at: None,
                created_by: None,
                updated_at: None,
                updated_by: None,
            })
            .await
            .map_err(database_error)?;
        Ok(true)
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[GarageFuellingMarkUseCase] {}", msg);
    BusinessError::new(msg)
}
