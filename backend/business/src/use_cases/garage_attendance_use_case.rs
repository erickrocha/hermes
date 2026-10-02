use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::bytes_para_string;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::{GarageAttendanceOrigin, GarageAttendanceStatus, GarageServiceState};
use crate::domain::garage_attendance::{GarageAttendance, GarageAttendanceEntityMapper};
use crate::domain::garage_service::{GarageService, GarageServiceEntityMapper};
use crate::domain::garage_service_model::GarageServiceModelEntityMapper;
use crate::domain::garage_service_log::GarageServiceLog;
use crate::gateway::garage_attendance_gateway::GarageAttendanceGateway;
use crate::gateway::garage_service_gateway::GarageServiceGateway;
use crate::gateway::garage_service_log_gateway::GarageServiceLogGateway;
use crate::gateway::garage_service_model_gateway::GarageServiceModelGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use chrono::NaiveDateTime;
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
        self.open_carrying(tenant_id, vehicle_id, manual_priority, origin, &[]).await
    }

    /// `EPIC-GA-06-S01` (`HRMS-968`, `TRM-414`/`416`/`417`): the triage a physical
    /// arrival opens by itself. `run` is the departure and arrival that just closed
    /// (`None` for a first arrival). Nothing happens -- and nothing fails -- when the
    /// vehicle already has a triage or the catalogue is empty (`TRM-414`).
    pub async fn open_on_arrival(
        &self,
        tenant_id: Option<i64>,
        vehicle_id: i64,
        run: Option<(NaiveDateTime, NaiveDateTime)>,
        min_trip_absence_minutes: i32,
    ) -> Result<Option<(GarageAttendance, Vec<GarageService>)>, BusinessError> {
        if self.attendances.find_active_by_vehicle(vehicle_id).await.map_err(database_error)?.is_some() {
            return Ok(None);
        }
        let from_trip = run.is_some_and(|(departed, arrived)| arrived - departed >= chrono::Duration::minutes(min_trip_absence_minutes as i64));
        let previous = match self.attendances.find_latest_by_vehicle(vehicle_id).await.map_err(database_error)? {
            Some(a) => GarageServiceEntityMapper::from_models(
                self.services.find_by_attendance(a.id).await.map_err(database_error)?,
            ),
            None => vec![],
        };
        let carried: Vec<GarageService> =
            previous.into_iter().filter(|s| carries_forward(s, from_trip, run.map(|(d, _)| d))).collect();
        match self.open_carrying(tenant_id, vehicle_id, None, GarageAttendanceOrigin::ArrivalAtBase, &carried).await {
            Ok(opened) => Ok(Some(opened)),
            Err(e) if e.message == NO_SERVICES || e.message == ALREADY_ACTIVE => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn open_carrying(
        &self,
        tenant_id: Option<i64>,
        vehicle_id: i64,
        manual_priority: Option<i32>,
        origin: GarageAttendanceOrigin,
        carried: &[GarageService],
    ) -> Result<(GarageAttendance, Vec<GarageService>), BusinessError> {
        let vehicle = self.vehicles.find_by_id(vehicle_id).await.map_err(database_error)?;
        let Some(vehicle) = vehicle.filter(|v| v.tenant_id == tenant_id) else {
            return Err(BusinessError::new(VEHICLE_NOT_FOUND.to_string()));
        };
        if self.attendances.find_active_by_vehicle(vehicle_id).await.map_err(database_error)?.is_some() {
            return Err(BusinessError::new(ALREADY_ACTIVE.to_string()));
        }
        let vehicle_uuid = bytes_para_string(vehicle.uuid);
        let vehicle_type = vehicle.vehicle_type;
        let catalogue = applicable_catalogue(
            GarageServiceModelEntityMapper::from_models(self.models.find_active().await.map_err(database_error)?),
            vehicle_type.as_deref(),
            &vehicle_uuid,
        );
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
            let Some(model_id) = model.id else {
                continue;
            };
            let before = carried.iter().find(|c| c.service_model_id == model_id);
            let saved = self
                .services
                .persist(GarageService {
                    id: None,
                    uuid: None,
                    tenant_id,
                    attendance_id: attendance.id.unwrap_or_default(),
                    service_model_id: model_id,
                    name_key: model.name_key,
                    state: before.map_or(GarageServiceState::Pending, |c| c.state),
                    performed_at: before.and_then(|c| c.performed_at),
                    marked_at: before.and_then(|c| c.marked_at),
                    forced_pending_at: before.and_then(|c| c.forced_pending_at),
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
/// `TRM-416`/`417`: whether a previous triage's service record is carried into the
/// new one. A vehicle that has not moved keeps its finished work; one that came back
/// from a trip starts clean (the return consumed what was recorded), and so does a
/// not-needed / not-done mark the vehicle's departure already consumed (`TRM-455`).
pub fn carries_forward(service: &GarageService, returned_from_trip: bool, departed_at: Option<NaiveDateTime>) -> bool {
    if returned_from_trip {
        return false;
    }
    match service.state {
        GarageServiceState::NotNeeded | GarageServiceState::NotDone => {
            !matches!((service.marked_at, departed_at), (Some(marked), Some(departed)) if marked < departed)
        }
        _ => true,
    }
}

fn unique_or_database_error(e: DbErr) -> BusinessError {
    if e.to_string().contains("uq_garage_attendance_active_vehicle") {
        return BusinessError::new(ALREADY_ACTIVE.to_string());
    }
    database_error(e)
}

fn applicable_catalogue(
    catalogue: Vec<crate::domain::garage_service_model::GarageServiceModel>,
    vehicle_type: Option<&str>,
    vehicle_uuid: &str,
) -> Vec<crate::domain::garage_service_model::GarageServiceModel> {
    catalogue
        .into_iter()
        .filter(|model| model.applicability.applies_to(vehicle_type, vehicle_uuid))
        .collect()
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[GarageAttendanceUseCase] {}", msg);
    BusinessError::new(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use crate::domain::garage_service_model::GarageServiceApplicability;

    fn at(h: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, 30).unwrap().and_hms_opt(h, 0, 0).unwrap()
    }

    fn service(state: GarageServiceState, marked_at: Option<NaiveDateTime>) -> GarageService {
        GarageService {
            id: None, uuid: None, tenant_id: None, attendance_id: 1, service_model_id: 1, name_key: "x".into(), state,
            performed_at: None, marked_at, forced_pending_at: None, created_at: None, created_by: None, updated_at: None, updated_by: None,
        }
    }

    fn catalogue_service(
        id: i64,
        applicability: GarageServiceApplicability,
    ) -> crate::domain::garage_service_model::GarageServiceModel {
        crate::domain::garage_service_model::GarageServiceModel {
            id: Some(id),
            uuid: None,
            tenant_id: Some(42),
            name: format!("service-{id}"),
            name_key: format!("service-{id}"),
            display_order: id as i32,
            active: true,
            service_group: crate::domain::enums::GarageServiceGroup::External,
            required_for_departure: false,
            governed_by_tank: false,
            applicability,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }
    }

    #[test]
    fn a_triage_catalogue_contains_only_services_applicable_to_its_vehicle() {
        let catalogue = vec![
            catalogue_service(1, GarageServiceApplicability::All),
            catalogue_service(2, GarageServiceApplicability::VehicleTypes(vec!["Ônibus".into()])),
            catalogue_service(3, GarageServiceApplicability::VehicleTypes(vec!["Van".into()])),
            catalogue_service(4, GarageServiceApplicability::Vehicles(vec!["vehicle-uuid".into()])),
        ];
        let applicable = applicable_catalogue(catalogue, Some("onibus"), "vehicle-uuid");
        assert_eq!(applicable.iter().map(|model| model.id).collect::<Vec<_>>(), [Some(1), Some(2), Some(4)]);
    }

    #[test]
    fn a_vehicle_that_has_not_moved_keeps_its_work_and_one_back_from_a_trip_starts_clean() {
        let done = service(GarageServiceState::Performed, None);
        assert!(carries_forward(&done, false, Some(at(8))), "a short absence carries (TRM-416)");
        assert!(!carries_forward(&done, true, Some(at(8))), "a return from a trip does not (TRM-417)");
        let not_needed = service(GarageServiceState::NotNeeded, Some(at(7)));
        assert!(!carries_forward(&not_needed, false, Some(at(8))), "a mark before the departure was consumed by it");
        assert!(carries_forward(&service(GarageServiceState::NotNeeded, Some(at(9))), false, Some(at(8))));
        assert!(carries_forward(&service(GarageServiceState::Pending, None), false, None));
    }
}
