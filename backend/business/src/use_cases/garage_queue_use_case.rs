use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::garage_attendance::{GarageAttendance, GarageAttendanceEntityMapper};
use crate::domain::enums::GarageServiceState;
use crate::domain::garage_service::{GarageService, GarageServiceEntityMapper};
use crate::domain::tenant_rule_setting::RuleSettings;
use crate::gateway::garage_attendance_gateway::GarageAttendanceGateway;
use crate::gateway::garage_service_gateway::GarageServiceGateway;
use crate::gateway::garage_service_model_gateway::GarageServiceModelGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use crate::use_cases::effective_schedule_use_case::EffectiveScheduleUseCase;
use crate::use_cases::garage_call_use_case::GarageCallUseCase;
use crate::use_cases::garage_readiness::{CallReasons, CardKey, MonitorKey, QueueKey, fill_tank_alert, tank_calls_vehicle, horizon_end, order_calls, order_cards, order_columns, order_monitor, order_queue, required_pending};
use crate::use_cases::garage_validity::TankFact;
use crate::use_cases::garage_validity_use_case::GarageValidityUseCase;
use chrono::NaiveDateTime;
use sea_orm::DbErr;
use std::collections::HashSet;

/// One vehicle in the yard's queue.
#[derive(Debug, Clone)]
pub struct QueueEntry {
    pub attendance: GarageAttendance,
    pub vehicle_uuid: String,
    pub prefix: String,
    pub next_departure: Option<NaiveDateTime>,
    /// Whether that departure is a real trip rather than a line or charter.
    pub next_is_trip: bool,
    /// Any service still effectively pending, required or not (`TRM-499`).
    pub outstanding: bool,
    /// The yard's rules, for the monitor's windows (`TRM-498`/`499`).
    pub rules: RuleSettings,
    /// Names of the required services still effectively pending (`TRM-469…471`).
    pub required_pending: Vec<String>,
    /// `TRM-474`: visual only, it never reorders the queue (`TRM-476`).
    pub fill_tank_alert: bool,
    /// `TRM-485`: a manual call in force.
    pub called_by_manager: bool,
}

/// `EPIC-GA-07-S04`: one service column of the matrix.
#[derive(Debug, Clone)]
pub struct MatrixColumn {
    pub uuid: String,
    pub name: String,
    pub internal: bool,
}

/// One cell: the service's effective state, the stamp of its stored state, and --
/// for a pending one -- the last time it was performed on this vehicle (`TRM-496`).
#[derive(Debug, Clone)]
pub struct MatrixCell {
    pub effective_state: GarageServiceState,
    pub stamp: Option<NaiveDateTime>,
    pub last_performed_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct MatrixRow {
    pub vehicle_uuid: String,
    pub prefix: String,
    /// One per column, `None` where the triage has no record of that service.
    pub cells: Vec<Option<MatrixCell>>,
}

#[derive(Debug, Clone)]
pub struct Matrix {
    pub columns: Vec<MatrixColumn>,
    pub rows: Vec<MatrixRow>,
    pub rows_not_shown: usize,
    pub columns_not_shown: usize,
}

/// `EPIC-GA-07-S03`: an away vehicle on the call-to-base list.
#[derive(Debug, Clone)]
pub struct CallEntry {
    pub vehicle_uuid: String,
    pub prefix: String,
    pub reasons: CallReasons,
    pub next_departure: Option<NaiveDateTime>,
}

/// `EPIC-GA-07-S02`: a vehicle card on the monitor.
#[derive(Debug, Clone)]
pub struct Card {
    pub vehicle_uuid: String,
    pub prefix: String,
    pub tank_percent: Option<f64>,
    pub needs_refuel: bool,
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
    models: GarageServiceModelGateway,
}

impl GarageQueueUseCase {
    pub fn new(
        attendances: GarageAttendanceGateway,
        services: GarageServiceGateway,
        schedule: EffectiveScheduleUseCase,
        vehicles: VehicleGateway,
        validity: GarageValidityUseCase,
        calls: GarageCallUseCase,
        models: GarageServiceModelGateway,
    ) -> Self {
        Self { attendances, services, schedule, vehicles, validity, calls, models }
    }

    /// `EPIC-GA-07-S01` (`HRMS-964`, `TRM-497`): the same facts, in the monitor's
    /// own order -- imminent work first, finished vehicles last.
    pub async fn monitor(&self) -> Result<Vec<QueueEntry>, BusinessError> {
        let now = chrono::Utc::now().naive_utc();
        let entries = self.queue().await?;
        let Some(rules) = entries.first().map(|e| e.rules) else { return Ok(entries) };
        let keys: Vec<MonitorKey> = entries
            .iter()
            .map(|e| MonitorKey {
                manual_priority: e.attendance.manual_priority,
                next_departure: e.next_departure.map(|at| (at, e.next_is_trip)),
                outstanding: e.outstanding,
                prefix: e.prefix.clone(),
            })
            .collect();
        let mut slots: Vec<Option<QueueEntry>> = entries.into_iter().map(Some).collect();
        Ok(order_monitor(&keys, now, &rules).into_iter().filter_map(|i| slots[i].take()).collect())
    }

    /// `EPIC-GA-07-S02` (`HRMS-965`, `TRM-495`/`1512`): the vehicles in the garage as
    /// cards -- need-to-refuel first, then the lowest tank -- capped at the tenant's
    /// limit, with how many are not shown.
    pub async fn cards(&self) -> Result<(Vec<Card>, usize), BusinessError> {
        let entries = self.queue().await?;
        let mut cards = Vec::with_capacity(entries.len());
        let mut limit = RuleSettings::default().garage_monitor_card_limit;
        for e in &entries {
            limit = e.rules.garage_monitor_card_limit;
            let vehicle_id = e.attendance.vehicle_id;
            let tank = self.validity.tank_fact(vehicle_id).await;
            let percent = match tank {
                TankFact::Percent(p) => Some(p),
                TankFact::NoUsableReading => None,
            };
            // Same alert as every other surface; a vehicle with a real trip ahead uses the trip cut.
            cards.push(Card {
                vehicle_uuid: e.vehicle_uuid.clone(),
                prefix: e.prefix.clone(),
                tank_percent: percent,
                needs_refuel: fill_tank_alert(tank, e.next_is_trip, &e.rules),
            });
        }
        let keys: Vec<CardKey> = cards
            .iter()
            .map(|c| CardKey { needs_refuel: c.needs_refuel, tank_percent: c.tank_percent, prefix: c.prefix.clone() })
            .collect();
        let mut slots: Vec<Option<Card>> = cards.into_iter().map(Some).collect();
        let ordered: Vec<Card> = order_cards(&keys).into_iter().filter_map(|i| slots[i].take()).collect();
        let hidden = ordered.len().saturating_sub(limit.max(0) as usize);
        Ok((ordered.into_iter().take(limit.max(0) as usize).collect(), hidden))
    }

    /// `EPIC-GA-07-S03` (`HRMS-966`, `TRM-485`/`487`/`1504`): the away vehicles that
    /// should come back -- a manual call, a further departure ahead, or a tank that
    /// calls on its own. Nothing here changes a tag: only the tracker confirms arrival.
    pub async fn call_list(&self) -> Result<Vec<CallEntry>, BusinessError> {
        let now = chrono::Utc::now().naive_utc();
        let today = now.date();
        let departures_of = self.schedule.departures_by_vehicle(today, horizon_end(today)).await?;
        let tank_service_model_ids: HashSet<i64> = self
            .models
            .find_all()
            .await
            .map_err(database_error)?
            .into_iter()
            .filter(|model| model.governed_by_tank)
            .map(|model| model.id)
            .collect();
        let mut entries = Vec::new();
        for vehicle in self.vehicles.find_all().await.map_err(database_error)? {
            let id = vehicle.id;
            if !self.calls.is_away(id).await? {
                continue;
            }
            let departures = departures_of.get(&id).cloned().unwrap_or_default();
            let ahead = departures.iter().filter(|d| d.at > now).min_by_key(|d| d.at).cloned();
            let rules = self.validity.rules_for(vehicle.tenant_id).await?;
            let tank = self.validity.tank_fact(id).await;
            let alert = fill_tank_alert(tank, ahead.as_ref().is_some_and(|d| d.is_trip), &rules);
            let fuelling_resolved = if alert {
                latest_triage_has_resolved_tank_service(
                    &self.attendances,
                    &self.services,
                    id,
                    &tank_service_model_ids,
                )
                .await?
            } else {
                false
            };
            let reasons = CallReasons {
                manual: self.calls.active(id).await?.is_some(),
                departure: ahead.is_some(),
                tank: tank_calls_vehicle(alert, fuelling_resolved, tank, &rules),
            };
            if reasons.any() {
                entries.push(CallEntry {
                    vehicle_uuid: crate::commons::functions::bytes_para_string(vehicle.uuid),
                    prefix: vehicle.prefix.unwrap_or(vehicle.plate),
                    reasons,
                    next_departure: ahead.map(|d| d.at),
                });
            }
        }
        let keys: Vec<_> = entries.iter().map(|e| (e.reasons, e.next_departure, e.prefix.clone())).collect();
        let mut slots: Vec<Option<CallEntry>> = entries.into_iter().map(Some).collect();
        Ok(order_calls(&keys).into_iter().filter_map(|i| slots[i].take()).collect())
    }

    /// `EPIC-GA-07-S04` (`HRMS-967`, `TRM-496`): active triages against the catalogue's
    /// services, external before internal, capped at the tenant's matrix size. Rows
    /// follow the monitor's order.
    pub async fn matrix(&self) -> Result<Matrix, BusinessError> {
        let entries = self.monitor().await?;
        let rules = entries.first().map(|e| e.rules).unwrap_or_default();
        let mut models = self.models.find_active().await.map_err(database_error)?;
        let order = order_columns(&models.iter().map(|m| (m.service_group == "Internal", m.display_order)).collect::<Vec<_>>());
        let mut slots: Vec<Option<_>> = models.drain(..).map(Some).collect();
        let models: Vec<_> = order.into_iter().filter_map(|i| slots[i].take()).collect();
        let total_columns = models.len();
        let shown_columns = models.len().min(rules.garage_monitor_matrix_columns.max(0) as usize);
        let models = &models[..shown_columns];
        let shown_rows = entries.len().min(rules.garage_monitor_matrix_rows.max(0) as usize);
        let mut rows = Vec::with_capacity(shown_rows);
        for entry in &entries[..shown_rows] {
            let attendance = &entry.attendance;
            let records = GarageServiceEntityMapper::from_models(
                self.services.find_by_attendance(attendance.id.unwrap_or_default()).await.map_err(database_error)?,
            );
            let evaluation = self.validity.evaluate_full(attendance, &records).await?;
            let history = self.attendances.find_ids_by_vehicle(attendance.vehicle_id).await.map_err(database_error)?;
            let performed = self.services.find_performed_by_attendances(history).await.map_err(database_error)?;
            let cells = models
                .iter()
                .map(|m| {
                    let i = records.iter().position(|r| Some(r.service_model_id) == Some(m.id))?;
                    let (effective, _) = evaluation.services[i];
                    let record = &records[i];
                    Some(MatrixCell {
                        effective_state: effective,
                        stamp: match record.state {
                            GarageServiceState::Performed => record.performed_at,
                            GarageServiceState::Pending => record.forced_pending_at,
                            _ => record.marked_at,
                        },
                        last_performed_at: (effective == GarageServiceState::Pending)
                            .then(|| performed.iter().find(|p| p.service_model_id == m.id).map(|p| p.performed_at.map(|t| t.naive_utc())))
                            .flatten()
                            .flatten(),
                    })
                })
                .collect();
            rows.push(MatrixRow { vehicle_uuid: entry.vehicle_uuid.clone(), prefix: entry.prefix.clone(), cells });
        }
        Ok(Matrix {
            columns: models
                .iter()
                .map(|m| MatrixColumn {
                    uuid: crate::commons::functions::bytes_para_string(m.uuid.clone()),
                    name: m.name.clone(),
                    internal: m.service_group == "Internal",
                })
                .collect(),
            rows,
            rows_not_shown: entries.len() - shown_rows,
            columns_not_shown: total_columns - shown_columns,
        })
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
                next_is_trip: departures.first().is_some_and(|d| d.is_trip),
                outstanding: evaluation.services.iter().any(|(state, _)| *state == GarageServiceState::Pending),
                rules: evaluation.rules,
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

fn has_resolved_tank_service(services: &[GarageService], tank_model_ids: &HashSet<i64>) -> bool {
    services.iter().any(|service| {
        tank_model_ids.contains(&service.service_model_id) && service.state != GarageServiceState::Pending
    })
}

async fn latest_triage_has_resolved_tank_service(
    attendances: &GarageAttendanceGateway,
    services: &GarageServiceGateway,
    vehicle_id: i64,
    tank_model_ids: &HashSet<i64>,
) -> Result<bool, BusinessError> {
    let Some(attendance) = attendances
        .find_latest_by_vehicle(vehicle_id)
        .await
        .map_err(database_error)?
    else {
        return Ok(false);
    };
    let records = GarageServiceEntityMapper::from_models(
        services.find_by_attendance(attendance.id).await.map_err(database_error)?,
    );
    Ok(has_resolved_tank_service(&records, tank_model_ids))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service(service_model_id: i64, state: GarageServiceState) -> GarageService {
        GarageService {
            id: None,
            uuid: None,
            tenant_id: Some(42),
            attendance_id: 1,
            service_model_id,
            name_key: "service".into(),
            state,
            performed_at: None,
            marked_at: None,
            forced_pending_at: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }
    }

    #[test]
    fn only_a_resolved_tank_governed_service_suppresses_the_tank_call() {
        let tank_models = HashSet::from([7]);
        assert!(!has_resolved_tank_service(&[service(7, GarageServiceState::Pending)], &tank_models));
        assert!(!has_resolved_tank_service(&[service(8, GarageServiceState::Performed)], &tank_models));
        assert!(has_resolved_tank_service(&[service(7, GarageServiceState::Performed)], &tank_models));
    }
}

#[cfg(all(test, feature = "mock"))]
mod gateway_tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone, Utc};
    use entity::audit::{AuditUser, run_with_user};
    use sea_orm::{DatabaseBackend, MockDatabase};

    fn at() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 2, 12, 0, 0).unwrap()
    }

    fn attendance() -> entity::garage_attendance_entity::Model {
        entity::garage_attendance_entity::Model {
            id: 8,
            uuid: vec![8; 16],
            tenant_id: Some(42),
            vehicle_id: 11,
            attendance_date: NaiveDate::from_ymd_opt(2026, 10, 2).unwrap(),
            checked_in_at: at(),
            status: "Finished".into(),
            manual_priority: None,
            released_at: Some(at()),
            origin: "Manual".into(),
            active_marker: None,
            created_at: at(),
            created_by: None,
            updated_at: at(),
            updated_by: None,
        }
    }

    fn service() -> entity::garage_service_entity::Model {
        entity::garage_service_entity::Model {
            id: 10,
            uuid: vec![10; 16],
            tenant_id: Some(42),
            attendance_id: 8,
            service_model_id: 7,
            name_key: "abastecimento".into(),
            state: "Performed".into(),
            performed_at: Some(at()),
            marked_at: None,
            forced_pending_at: None,
            created_at: at(),
            created_by: None,
            updated_at: at(),
            updated_by: None,
        }
    }

    #[tokio::test]
    async fn resolved_fuel_service_is_read_from_the_vehicle_latest_triage() {
        let db = MockDatabase::new(DatabaseBackend::MySql)
            .append_query_results([[attendance()]])
            .append_query_results([[service()]])
            .into_connection();
        let attendances = GarageAttendanceGateway::new(db.clone());
        let services = GarageServiceGateway::new(db);
        let user = AuditUser {
            id: 2,
            email: "owner@example.com".into(),
            tenant_id: Some(42),
            enforce_tenant: true,
        };

        let resolved = run_with_user(
            Some(user),
            latest_triage_has_resolved_tank_service(&attendances, &services, 11, &HashSet::from([7])),
        )
        .await
        .expect("latest triage was read");

        assert!(resolved);
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[GarageQueueUseCase] {}", msg);
    BusinessError::new(msg)
}
