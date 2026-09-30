use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::GarageServiceState;
use crate::domain::garage_attendance::GarageAttendance;
use crate::domain::garage_service::GarageService;
use crate::domain::tenant_rule_setting::RuleSettings;
use crate::gateway::extra_trip_gateway::ExtraTripGateway;
use crate::gateway::garage_service_model_gateway::GarageServiceModelGateway;
use crate::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway;
use crate::gateway::vehicle_presence_event_gateway::VehiclePresenceEventGateway;
use crate::use_cases::fuel_gauge_use_case::{FuelGaugeUseCase, TankGauge};
use crate::use_cases::garage_validity::{
    PresenceFacts, ServiceFacts, TankFact, ValidityContext, ValidityReason, effective_state,
};
use chrono::NaiveDateTime;
use sea_orm::DbErr;

/// What a triage's services evaluate to, and what they were evaluated on.
pub struct Evaluation {
    pub services: Vec<(GarageServiceState, ValidityReason)>,
    /// The catalogue's explicit required-for-departure flag, per service (`TRM-470`).
    pub required: Vec<bool>,
    pub names: Vec<String>,
    pub tank: TankFact,
    pub rules: RuleSettings,
}

/// `EPIC-GA-04` (`HRMS-961`): gathers the facts the validity rule reads -- the
/// vehicle's physical stamps, its latest marked trip, the tank reading -- and
/// evaluates every service of a triage with them. Nothing is stored
/// (`TRM-447`); identical facts feed every surface (`TRM-448`).
pub struct GarageValidityUseCase {
    events: VehiclePresenceEventGateway,
    trips: ExtraTripGateway,
    models: GarageServiceModelGateway,
    rules: TenantRuleSettingGateway,
    gauge: FuelGaugeUseCase,
}

impl GarageValidityUseCase {
    pub fn new(
        events: VehiclePresenceEventGateway,
        trips: ExtraTripGateway,
        models: GarageServiceModelGateway,
        rules: TenantRuleSettingGateway,
        gauge: FuelGaugeUseCase,
    ) -> Self {
        Self { events, trips, models, rules, gauge }
    }

    /// The effective state and reason of each of the triage's services, in
    /// the order given. A closed triage is history: its stored states stand.
    pub async fn evaluate(
        &self,
        attendance: &GarageAttendance,
        services: &[GarageService],
    ) -> Result<Vec<(GarageServiceState, ValidityReason)>, BusinessError> {
        Ok(self.evaluate_full(attendance, services).await?.services)
    }

    /// The same, with the facts it was decided on -- so a caller that also needs
    /// the tank reading or the catalogue flags does not read them twice.
    pub async fn evaluate_full(
        &self,
        attendance: &GarageAttendance,
        services: &[GarageService],
    ) -> Result<Evaluation, BusinessError> {
        use crate::domain::enums::GarageAttendanceStatus;
        if attendance.status != GarageAttendanceStatus::Open {
            return Ok(Evaluation {
                services: services.iter().map(|s| (s.state, ValidityReason::Valid)).collect(),
                required: vec![false; services.len()],
                names: vec![String::new(); services.len()],
                tank: TankFact::NoUsableReading,
                rules: RuleSettings::default(),
            });
        }
        let vehicle_id = attendance.vehicle_id;
        let rules: RuleSettings = self.rules.settings_for(attendance.tenant_id).await.map_err(database_error)?;
        let presence = self.presence(vehicle_id).await?;
        let trip_marked_at = self
            .trips
            .find_upcoming_by_vehicle(vehicle_id, chrono::Utc::now().date_naive())
            .await
            .map_err(database_error)?
            .first()
            .map(|t| t.created_at.naive_utc());

        // One gauge read, and only if some service is tank-governed.
        let mut governed = Vec::with_capacity(services.len());
        let mut required = Vec::with_capacity(services.len());
        let mut names = Vec::with_capacity(services.len());
        for s in services {
            let model = self.models.find_by_id(s.service_model_id).await.map_err(database_error)?;
            governed.push(model.as_ref().is_some_and(|m| m.governed_by_tank));
            required.push(model.as_ref().is_some_and(|m| m.required_for_departure));
            names.push(model.map(|m| m.name).unwrap_or_else(|| s.name_key.clone()));
        }
        let tank = if governed.iter().any(|g| *g) {
            match self.gauge.gauge(vehicle_id).await? {
                TankGauge::Reading { percent, .. } => TankFact::Percent(percent),
                TankGauge::Unavailable(_) => TankFact::NoUsableReading,
            }
        } else {
            TankFact::NoUsableReading
        };

        let ctx = ValidityContext { presence, trip_marked_at, tank, now: chrono::Utc::now().naive_utc() };
        let effective = services
            .iter()
            .zip(governed)
            .map(|(s, governed_by_tank)| {
                effective_state(
                    &ServiceFacts {
                        state: s.state,
                        performed_at: s.performed_at,
                        marked_at: s.marked_at,
                        forced_pending_at: s.forced_pending_at,
                        governed_by_tank,
                    },
                    &ctx,
                    &rules,
                )
            })
            .collect();
        Ok(Evaluation { services: effective, required, names, tank, rules })
    }

    async fn presence(&self, vehicle_id: i64) -> Result<PresenceFacts, BusinessError> {
        let at = |m: Option<entity::vehicle_presence_event_entity::Model>| m.map(|m| m.occurred_at.naive_utc());
        let last_arrival_at = at(self.events.find_latest_of_kind(vehicle_id, "Arrival").await.map_err(database_error)?);
        let last_departure_at = at(self.events.find_latest_of_kind(vehicle_id, "Departure").await.map_err(database_error)?);
        let away = match (last_arrival_at, last_departure_at) {
            (_, None) => false,
            (None, Some(_)) => true,
            (Some(arrival), Some(departure)) => departure > arrival,
        };
        // The latest completed run: the last arrival and the departure before it.
        let last_run = match last_arrival_at {
            Some(arrival) => self
                .departure_before(vehicle_id, arrival)
                .await?
                .map(|departed| (departed, arrival)),
            None => None,
        };
        Ok(PresenceFacts { last_arrival_at, last_departure_at, away, last_run })
    }

    async fn departure_before(&self, vehicle_id: i64, before: NaiveDateTime) -> Result<Option<NaiveDateTime>, BusinessError> {
        Ok(self
            .events
            .find_latest_of_kind_before(vehicle_id, "Departure", before)
            .await
            .map_err(database_error)?
            .map(|m| m.occurred_at.naive_utc()))
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[GarageValidityUseCase] {}", msg);
    BusinessError::new(msg)
}
