use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::{GarageAttendanceOrigin, GarageAttendanceStatus, GarageServiceState};
use crate::domain::garage_attendance::{GarageAttendance, GarageAttendanceEntityMapper};
use crate::domain::garage_service::{GarageService, GarageServiceEntityMapper};
use crate::domain::garage_service_log::GarageServiceLog;
use crate::gateway::garage_attendance_gateway::GarageAttendanceGateway;
use crate::gateway::garage_service_gateway::GarageServiceGateway;
use crate::gateway::garage_service_log_gateway::GarageServiceLogGateway;
use crate::gateway::garage_service_model_gateway::GarageServiceModelGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use sea_orm::DbErr;

pub const VEHICLE_NOT_FOUND: &str = "Vehicle not found";
pub const ALREADY_ACTIVE: &str = "This vehicle already has an active triage";
pub const NO_SERVICES: &str = "The garage catalogue has no active service to open a triage against";
pub const ATTENDANCE_NOT_FOUND: &str = "Triage not found";
pub const SERVICE_NOT_FOUND: &str = "Service not found on this triage";
pub const ATTENDANCE_CLOSED: &str = "This triage is already closed";

/// `EPIC-GA-02-S01`/`S02` (`HRMS-958`/`959`): triage and service records.
pub struct GarageAttendanceUseCase {
    attendances: GarageAttendanceGateway,
    services: GarageServiceGateway,
    logs: GarageServiceLogGateway,
    models: GarageServiceModelGateway,
    vehicles: VehicleGateway,
}

impl GarageAttendanceUseCase {
    pub fn new(
        attendances: GarageAttendanceGateway,
        services: GarageServiceGateway,
        logs: GarageServiceLogGateway,
        models: GarageServiceModelGateway,
        vehicles: VehicleGateway,
    ) -> Self {
        Self { attendances, services, logs, models, vehicles }
    }

    /// `TRM-410`/`412`/`414`: opens a triage for a vehicle with one pending
    /// service record per active catalogue entry. A vehicle holds at most one
    /// active triage -- the pre-check gives the caller a clear answer, the
    /// unique (vehicle, active marker) index stops a race. A triage against an
    /// empty catalogue is refused: it would have nothing to record.
    pub async fn open(
        &self,
        tenant_id: Option<i64>,
        vehicle_id: i64,
        manual_priority: Option<i32>,
        origin: GarageAttendanceOrigin,
    ) -> Result<(GarageAttendance, Vec<GarageService>), BusinessError> {
        let vehicle = self.vehicles.find_by_id(vehicle_id).await.map_err(database_error)?;
        if !vehicle.is_some_and(|v| v.tenant_id == tenant_id) {
            return Err(BusinessError::new(VEHICLE_NOT_FOUND.to_string()));
        }
        if self.attendances.find_active_by_vehicle(vehicle_id).await.map_err(database_error)?.is_some() {
            return Err(BusinessError::new(ALREADY_ACTIVE.to_string()));
        }
        let catalogue = self.models.find_active().await.map_err(database_error)?;
        if catalogue.is_empty() {
            return Err(BusinessError::new(NO_SERVICES.to_string()));
        }

        let now = chrono::Utc::now().naive_utc();
        let saved = self
            .attendances
            .persist(GarageAttendance {
                id: None,
                uuid: None,
                tenant_id,
                vehicle_id,
                attendance_date: now.date(),
                checked_in_at: now,
                status: GarageAttendanceStatus::Open,
                manual_priority,
                released_at: None,
                origin,
                active_marker: Some(1),
                created_at: None,
                created_by: None,
                updated_at: None,
                updated_by: None,
            })
            .await
            .map_err(unique_or_database_error)?;
        let attendance = GarageAttendanceEntityMapper::from_active_model(saved);

        let mut services = Vec::with_capacity(catalogue.len());
        for model in catalogue {
            let saved = self
                .services
                .persist(GarageService {
                    id: None,
                    uuid: None,
                    tenant_id,
                    attendance_id: attendance.id.unwrap_or_default(),
                    service_model_id: model.id,
                    name_key: model.name_key,
                    state: GarageServiceState::Pending,
                    performed_at: None,
                    marked_at: None,
                    forced_pending_at: None,
                    created_at: None,
                    created_by: None,
                    updated_at: None,
                    updated_by: None,
                })
                .await
                .map_err(database_error)?;
            services.push(GarageServiceEntityMapper::from_active_model(saved));
        }
        Ok((attendance, services))
    }

    /// `TRM-437`/`438`/`440`: marks one service of an **active** triage. Each
    /// state writes its own stamp -- the performance instant for performed, the
    /// marking instant for not-needed and not-done, the forcing instant for a
    /// return to pending -- and an independent audit row is written with the
    /// marking. Only the stamp the state owns changes; a service's last
    /// performance survives a later mark, as the "last performed" display needs.
    pub async fn mark_service(
        &self,
        attendance_uuid: String,
        service_uuid: String,
        state: GarageServiceState,
        acted_by_user_id: Option<i64>,
    ) -> Result<GarageService, BusinessError> {
        let (attendance, _) = self.find_by_uuid(attendance_uuid).await?;
        if attendance.status != GarageAttendanceStatus::Open {
            return Err(BusinessError::new(ATTENDANCE_CLOSED.to_string()));
        }
        let attendance_id = attendance.id.unwrap_or_default();
        let service = self
            .services
            .find_by_uuid(service_uuid)
            .await
            .map_err(database_error)?
            .map(GarageServiceEntityMapper::from_model)
            .filter(|s| s.attendance_id == attendance_id)
            .ok_or_else(|| BusinessError::new(SERVICE_NOT_FOUND.to_string()))?;

        let now = chrono::Utc::now().naive_utc();
        let updated = GarageService {
            state,
            performed_at: if state == GarageServiceState::Performed { Some(now) } else { service.performed_at },
            marked_at: match state {
                GarageServiceState::NotNeeded | GarageServiceState::NotDone => Some(now),
                GarageServiceState::Performed => None,
                GarageServiceState::Pending => service.marked_at,
            },
            forced_pending_at: match state {
                GarageServiceState::Pending => Some(now),
                GarageServiceState::Performed => None,
                _ => service.forced_pending_at,
            },
            ..service
        };
        let saved = self.services.persist(updated).await.map_err(database_error)?;
        let saved = GarageServiceEntityMapper::from_active_model(saved);

        self.logs
            .persist(GarageServiceLog {
                id: None,
                uuid: None,
                tenant_id: saved.tenant_id,
                attendance_id,
                vehicle_id: attendance.vehicle_id,
                service_model_id: saved.service_model_id,
                name_key: saved.name_key.clone(),
                new_state: state,
                acted_by_user_id,
                acted_at: now,
                origin: "Employee".to_string(),
                created_at: None,
                created_by: None,
                updated_at: None,
                updated_by: None,
            })
            .await
            .map_err(database_error)?;
        Ok(saved)
    }

    /// `TRM-418`/`423`/`425`: an administrative check-out closes the triage as
    /// finished, records the release time, frees the active slot and zeroes
    /// the manual priority, so its old position never survives a return.
    pub async fn checkout(&self, attendance_uuid: String) -> Result<GarageAttendance, BusinessError> {
        let (attendance, _) = self.find_by_uuid(attendance_uuid).await?;
        if attendance.status != GarageAttendanceStatus::Open {
            return Err(BusinessError::new(ATTENDANCE_CLOSED.to_string()));
        }
        let saved = self
            .attendances
            .persist(GarageAttendance {
                status: GarageAttendanceStatus::Finished,
                released_at: Some(chrono::Utc::now().naive_utc()),
                active_marker: None,
                manual_priority: None,
                ..attendance
            })
            .await
            .map_err(database_error)?;
        Ok(GarageAttendanceEntityMapper::from_active_model(saved))
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<(GarageAttendance, Vec<GarageService>), BusinessError> {
        let attendance = self
            .attendances
            .find_by_uuid(uuid)
            .await
            .map_err(database_error)?
            .map(GarageAttendanceEntityMapper::from_model)
            .ok_or_else(|| BusinessError::new(ATTENDANCE_NOT_FOUND.to_string()))?;
        let services = self.services_of(&attendance).await?;
        Ok((attendance, services))
    }

    /// `TRM-412`: the vehicle's active triage, if it has one.
    pub async fn find_active(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<(GarageAttendance, Vec<GarageService>)>, BusinessError> {
        let Some(model) = self.attendances.find_active_by_vehicle(vehicle_id).await.map_err(database_error)? else {
            return Ok(None);
        };
        let attendance = GarageAttendanceEntityMapper::from_model(model);
        let services = self.services_of(&attendance).await?;
        Ok(Some((attendance, services)))
    }

    async fn services_of(&self, attendance: &GarageAttendance) -> Result<Vec<GarageService>, BusinessError> {
        let rows = self.services.find_by_attendance(attendance.id.unwrap_or_default()).await.map_err(database_error)?;
        Ok(GarageServiceEntityMapper::from_models(rows))
    }
}

/// A race past the pre-check is still "already active", not a server fault.
fn unique_or_database_error(e: DbErr) -> BusinessError {
    if e.to_string().contains("uq_garage_attendance_active_vehicle") {
        return BusinessError::new(ALREADY_ACTIVE.to_string());
    }
    database_error(e)
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[GarageAttendanceUseCase] {}", msg);
    BusinessError::new(msg)
}
