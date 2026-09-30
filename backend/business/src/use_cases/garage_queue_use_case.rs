use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::garage_attendance::{GarageAttendance, GarageAttendanceEntityMapper};
use crate::domain::garage_service::GarageServiceEntityMapper;
use crate::gateway::garage_attendance_gateway::GarageAttendanceGateway;
use crate::gateway::garage_service_gateway::GarageServiceGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use crate::use_cases::effective_schedule_use_case::EffectiveScheduleUseCase;
use crate::use_cases::garage_call_use_case::GarageCallUseCase;
use crate::use_cases::garage_readiness::{QueueKey, fill_tank_alert, horizon_end, order_queue, required_pending};
use crate::use_cases::garage_validity_use_case::GarageValidityUseCase;
use chrono::NaiveDateTime;
use sea_orm::DbErr;

/// One vehicle in the yard's queue.
#[derive(Debug, Clone)]
pub struct QueueEntry {
    pub attendance: GarageAttendance,
    pub vehicle_uuid: String,
    pub prefix: String,
    pub next_departure: Option<NaiveDateTime>,
    /// Names of the required services still effectively pending (`TRM-469…471`).
    pub required_pending: Vec<String>,
    /// `TRM-474`: visual only, it never reorders the queue (`TRM-476`).
    pub fill_tank_alert: bool,
    /// `TRM-485`: a manual call in force.
    pub called_by_manager: bool,
}

/// `EPIC-GA-05-S01`/`S03`/`S04` (`HRMS-962`): the yard's queue -- every active
/// triage with what is left to do, in the one order every surface shows (`TRM-482`).
pub struct GarageQueueUseCase {
    attendances: GarageAttendanceGateway,
    services: GarageServiceGateway,
    schedule: EffectiveScheduleUseCase,
    vehicles: VehicleGateway,
    validity: GarageValidityUseCase,
    calls: GarageCallUseCase,
}

impl GarageQueueUseCase {
    pub fn new(
        attendances: GarageAttendanceGateway,
        services: GarageServiceGateway,
        schedule: EffectiveScheduleUseCase,
        vehicles: VehicleGateway,
        validity: GarageValidityUseCase,
        calls: GarageCallUseCase,
    ) -> Self {
        Self { attendances, services, schedule, vehicles, validity, calls }
    }

    pub async fn queue(&self) -> Result<Vec<QueueEntry>, BusinessError> {
        let today = chrono::Utc::now().date_naive();
        let horizon = horizon_end(today);
        // The horizon's days are read once for the whole yard (`TRM-001`, `TRM-481`).
        let departures_of = self.schedule.departures_by_vehicle(today, horizon).await?;
        let mut entries = Vec::new();
        for model in self.attendances.find_all_active().await.map_err(database_error)? {
            let attendance = GarageAttendanceEntityMapper::from_model(model);
            let vehicle = self.vehicles.find_by_id(attendance.vehicle_id).await.map_err(database_error)?;
            let Some(vehicle) = vehicle else { continue };
            let rows = self.services.find_by_attendance(attendance.id.unwrap_or_default()).await.map_err(database_error)?;
            let services = GarageServiceEntityMapper::from_models(rows);
            let evaluation = self.validity.evaluate_full(&attendance, &services).await?;

            // A line or charter alone is never "going to travel" (`TRM-475`): only a
            // real trip is. Both order the queue (`TRM-480`).
            let departures = departures_of.get(&attendance.vehicle_id).cloned().unwrap_or_default();
            let going_to_travel = departures.iter().any(|d| d.is_trip);

            let pairs: Vec<_> = evaluation.required.iter().copied().zip(evaluation.services.iter().map(|(s, _)| *s)).collect();
            let blocking = required_pending(&pairs).into_iter().map(|i| evaluation.names[i].clone()).collect();
            entries.push(QueueEntry {
                fill_tank_alert: fill_tank_alert(evaluation.tank, going_to_travel, &evaluation.rules),
                called_by_manager: self.calls.active(attendance.vehicle_id).await?.is_some(),
                next_departure: departures.first().map(|d| d.at),
                required_pending: blocking,
                vehicle_uuid: crate::commons::functions::bytes_para_string(vehicle.uuid),
                prefix: vehicle.prefix.unwrap_or_else(|| vehicle.plate.clone()),
                attendance,
            });
        }
        let keys: Vec<QueueKey> = entries
            .iter()
            .map(|e| QueueKey { manual_priority: e.attendance.manual_priority, next_departure: e.next_departure, prefix: e.prefix.clone() })
            .collect();
        let order = order_queue(&keys, today);
        let mut slots: Vec<Option<QueueEntry>> = entries.into_iter().map(Some).collect();
        Ok(order.into_iter().filter_map(|i| slots[i].take()).collect())
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[GarageQueueUseCase] {}", msg);
    BusinessError::new(msg)
}
