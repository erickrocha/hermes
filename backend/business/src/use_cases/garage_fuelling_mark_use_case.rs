use crate::domain::business_error::BusinessError;
use crate::domain::enums::GarageServiceState;
use crate::domain::garage_attendance::GarageAttendance;
use crate::domain::garage_service::GarageService;
use crate::domain::garage_service_log::GarageServiceLog;
use crate::gateway::garage_fuelling_mark_gateway::GarageFuellingMarkStore;
use crate::use_cases::garage_readiness::should_mark_fuelling;

/// `EPIC-GA-06-S02` (`HRMS-969`, `TRM-1520…1529`): a provider-reported full tank
/// marks the fuelling service of the vehicle's open triage by itself.
///
/// Idempotent by construction: once marked, the fuelling is no longer newer than
/// the service's confirmation, so running it again changes nothing. That is why
/// it can be called from every place a fuelling or a triage appears -- the
/// provider sync, and a triage opened hours after the fuelling (`TRM-1522`).
pub struct GarageFuellingMarkUseCase {
    store: Box<dyn GarageFuellingMarkStore>,
}

impl GarageFuellingMarkUseCase {
    pub fn new<S: GarageFuellingMarkStore + 'static>(store: S) -> Self {
        Self {
            store: Box::new(store),
        }
    }

    /// Every open triage of the caller's tenant (`TRM-1528`: only the current
    /// triage is ever touched). Returns how many services were marked.
    pub async fn apply_all_active(&self) -> Result<usize, BusinessError> {
        let mut marked = 0;
        for attendance in self.store.active_attendances().await? {
            if self.apply_to(attendance).await? {
                marked += 1;
            }
        }
        Ok(marked)
    }

    /// One vehicle's open triage. `false` when there is nothing to mark.
    pub async fn apply(&self, vehicle_id: i64) -> Result<bool, BusinessError> {
        match self.store.active_attendance(vehicle_id).await? {
            Some(attendance) => self.apply_to(attendance).await,
            None => Ok(false),
        }
    }

    async fn apply_to(&self, attendance: GarageAttendance) -> Result<bool, BusinessError> {
        let vehicle_id = attendance.vehicle_id;
        let Some(fuelling) = self.store.latest_full_tank_fuel(vehicle_id).await? else {
            return Ok(false);
        };
        let fuelled_at = fuelling.recorded_at;
        let records = self
            .store
            .services(attendance.id.unwrap_or_default())
            .await?;
        // The fuelling service is the catalogue's tank-governed one, never a name match (`TRM-462`).
        let mut fuelling_service: Option<GarageService> = None;
        for record in records {
            let model = self.store.service_model(record.service_model_id).await?;
            if model.is_some_and(|m| m.governed_by_tank) {
                fuelling_service = Some(record);
                break;
            }
        }
        let Some(service) = fuelling_service else {
            return Ok(false);
        };

        let rules = self.store.rules(attendance.tenant_id).await?;
        let last_arrival = self.store.last_arrival(vehicle_id).await?;
        let confirmation = service
            .performed_at
            .into_iter()
            .chain(service.marked_at)
            .max();
        let now = chrono::Utc::now().naive_utc();
        if !should_mark_fuelling(
            fuelled_at,
            now,
            confirmation,
            last_arrival,
            service.forced_pending_at,
            &rules,
        ) {
            return Ok(false);
        }

        // `TRM-1526`: the mark carries the fuelling's own instant, clears the forcing stamp.
        let saved = self
            .store
            .save_service(GarageService {
                state: GarageServiceState::Performed,
                performed_at: Some(fuelled_at),
                marked_at: None,
                forced_pending_at: None,
                ..service
            })
            .await?;
        self.store
            .save_log(GarageServiceLog {
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
                origin: format!("System: fuelling {}", fuelling.id.unwrap_or_default()),
                created_at: None,
                created_by: None,
                updated_at: None,
                updated_by: None,
            })
            .await?;
        Ok(true)
    }
}
