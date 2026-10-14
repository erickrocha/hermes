use crate::domain::business_error::BusinessError;
use crate::domain::enums::{GarageAttendanceStatus, GarageServiceState};
use crate::domain::garage_attendance::GarageAttendance;
use crate::domain::garage_service::GarageService;
use crate::gateway::garage_auto_departure_gateway::GarageAutoDepartureStore;
use crate::use_cases::effective_schedule::ItemKind;
use crate::use_cases::effective_schedule_use_case::Departure;
use crate::use_cases::effective_schedule_use_case::EffectiveScheduleUseCase;
use crate::use_cases::garage_readiness::required_pending;
use crate::use_cases::garage_validity_use_case::GarageValidityUseCase;
use chrono::NaiveDateTime;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AutomaticDepartureOutcome {
    pub closed: usize,
}

pub struct GarageAutoDepartureUseCase {
    store: Box<dyn GarageAutoDepartureStore>,
    schedule: EffectiveScheduleUseCase,
    validity: GarageValidityUseCase,
}

impl GarageAutoDepartureUseCase {
    pub fn new<S>(
        store: S,
        schedule: EffectiveScheduleUseCase,
        validity: GarageValidityUseCase,
    ) -> Self
    where
        S: GarageAutoDepartureStore + 'static,
    {
        Self {
            store: Box::new(store),
            schedule,
            validity,
        }
    }

    pub async fn run(&self) -> Result<AutomaticDepartureOutcome, BusinessError> {
        let now = chrono::Utc::now().naive_utc();
        let departures = self
            .schedule
            .departures_by_vehicle(now.date(), now.date())
            .await?;
        let mut outcome = AutomaticDepartureOutcome::default();
        for (attendance, services) in self.store.find_active_with_services().await? {
            let Some(departure) = departures
                .get(&attendance.vehicle_id)
                .and_then(|items| due_departure(&attendance, items, now))
                .cloned()
            else {
                continue;
            };
            let evaluation = self.validity.evaluate_full(&attendance, &services).await?;
            let pairs: Vec<_> = evaluation
                .required
                .iter()
                .copied()
                .zip(evaluation.services.iter().map(|(state, _)| *state))
                .collect();
            let pending = required_pending(&pairs);
            let service_names: Vec<String> = pending
                .iter()
                .filter_map(|index| evaluation.names.get(*index).cloned())
                .collect();
            let marked_services: Vec<GarageService> = pending
                .iter()
                .filter_map(|index| services.get(*index))
                .map(|service| GarageService {
                    state: GarageServiceState::NotDone,
                    marked_at: Some(departure.at),
                    ..service.clone()
                })
                .collect();
            let justification = automatic_departure_justification(&departure, &service_names);
            let updated = GarageAttendance {
                status: GarageAttendanceStatus::LeftForOperation,
                released_at: Some(departure.at),
                manual_priority: None,
                automatic_departure_justification: Some(justification),
                active_marker: None,
                ..attendance
            };
            if self
                .store
                .close_automatically(updated, marked_services)
                .await?
            {
                outcome.closed += 1;
            }
        }
        Ok(outcome)
    }
}

pub fn due_departure<'a>(
    attendance: &GarageAttendance,
    departures: &'a [Departure],
    now: NaiveDateTime,
) -> Option<&'a Departure> {
    if attendance.status != GarageAttendanceStatus::Open {
        return None;
    }
    departures
        .iter()
        .filter(|departure| {
            automatic_departure_type(departure.kind).is_some()
                && departure.at > attendance.checked_in_at
                && departure.at <= now
        })
        .min_by_key(|departure| departure.at)
}

pub fn automatic_departure_type(kind: ItemKind) -> Option<&'static str> {
    match kind {
        ItemKind::OneOffTrip => Some("viagem avulsa"),
        ItemKind::ExtraTrip => Some("viagem extra"),
        ItemKind::ExtraLine => Some("fretamento"),
        ItemKind::Line => None,
    }
}

pub fn automatic_departure_justification(
    departure: &Departure,
    unperformed_services: &[String],
) -> String {
    let kind = automatic_departure_type(departure.kind).unwrap_or("partida programada");
    let name = departure
        .name
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("sem nome");
    let services = if unperformed_services.is_empty() {
        "nenhum serviço obrigatório pendente".to_string()
    } else {
        unperformed_services.join(", ")
    };
    format!(
        "Saída automática para {kind} '{name}' às {}. Serviços obrigatórios não realizados: {services}.",
        departure.at.format("%Y-%m-%d %H:%M")
    )
}
