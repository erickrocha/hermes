use crate::domain::business_error::BusinessError;
use crate::domain::enums::{GarageServiceState, PresenceEventKind};
use crate::domain::garage_attendance::GarageAttendance;
use crate::domain::garage_service::GarageService;
use crate::domain::tenant_rule_setting::RuleSettings;
use crate::gateway::extra_trip_gateway::GarageValidityTripStore;
use crate::gateway::garage_service_model_gateway::GarageServiceModelStore;
use crate::gateway::tenant_rule_setting_gateway::TenantRuleSettingStore;
use crate::gateway::vehicle_presence_event_gateway::GarageValidityPresenceStore;
use crate::use_cases::fuel_gauge_use_case::{FuelGaugeReader, TankGauge};
use crate::use_cases::garage_validity::{
    PresenceFacts, ServiceFacts, TankFact, ValidityContext, ValidityReason, effective_state,
};
use chrono::NaiveDateTime;

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
    events: Box<dyn GarageValidityPresenceStore>,
    trips: Box<dyn GarageValidityTripStore>,
    models: Box<dyn GarageServiceModelStore>,
    rules: Box<dyn TenantRuleSettingStore>,
    gauge: Box<dyn FuelGaugeReader>,
}

impl GarageValidityUseCase {
    pub fn new<E, T, M, R, G>(events: E, trips: T, models: M, rules: R, gauge: G) -> Self
    where
        E: GarageValidityPresenceStore + 'static,
        T: GarageValidityTripStore + 'static,
        M: GarageServiceModelStore + 'static,
        R: TenantRuleSettingStore + 'static,
        G: FuelGaugeReader + 'static,
    {
        Self {
            events: Box::new(events),
            trips: Box::new(trips),
            models: Box::new(models),
            rules: Box::new(rules),
            gauge: Box::new(gauge),
        }
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
                services: services
                    .iter()
                    .map(|s| (s.state, ValidityReason::Valid))
                    .collect(),
                required: vec![false; services.len()],
                names: vec![String::new(); services.len()],
                tank: TankFact::NoUsableReading,
                rules: RuleSettings::default(),
            });
        }
        let vehicle_id = attendance.vehicle_id;
        let rules = self
            .rules
            .get_current(attendance.tenant_id)
            .await?
            .map(|setting| setting.settings())
            .unwrap_or_default();
        let presence = self.presence(vehicle_id).await?;
        let trip_marked_at = self
            .trips
            .upcoming_for_vehicle(vehicle_id, chrono::Utc::now().date_naive())
            .await?
            .first()
            .and_then(|trip| trip.created_at);

        // One gauge read, and only if some service is tank-governed.
        let mut governed = Vec::with_capacity(services.len());
        let mut required = Vec::with_capacity(services.len());
        let mut names = Vec::with_capacity(services.len());
        for s in services {
            let model = self.models.get_by_id(s.service_model_id).await?;
            governed.push(model.as_ref().is_some_and(|model| model.governed_by_tank));
            required.push(
                model
                    .as_ref()
                    .is_some_and(|model| model.required_for_departure),
            );
            names.push(
                model
                    .map(|model| model.name)
                    .unwrap_or_else(|| s.name_key.clone()),
            );
        }
        let tank = if governed.iter().any(|g| *g) {
            match self.gauge.read_gauge(vehicle_id).await? {
                TankGauge::Reading { percent, .. } => TankFact::Percent(percent),
                TankGauge::Unavailable(_) => TankFact::NoUsableReading,
            }
        } else {
            TankFact::NoUsableReading
        };

        let ctx = ValidityContext {
            presence,
            trip_marked_at,
            tank,
            now: chrono::Utc::now().naive_utc(),
        };
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
        Ok(Evaluation {
            services: effective,
            required,
            names,
            tank,
            rules,
        })
    }

    pub async fn rules_for(&self, tenant_id: Option<i64>) -> Result<RuleSettings, BusinessError> {
        Ok(self
            .rules
            .get_current(tenant_id)
            .await?
            .map(|setting| setting.settings())
            .unwrap_or_default())
    }

    /// The vehicle's tank reading whether or not any service is tank-governed
    /// (the monitor's cards show every vehicle's tank, `TRM-495`).
    pub async fn tank_fact(&self, vehicle_id: i64) -> TankFact {
        match self.gauge.read_gauge(vehicle_id).await {
            Ok(TankGauge::Reading { percent, .. }) => TankFact::Percent(percent),
            _ => TankFact::NoUsableReading,
        }
    }

    async fn presence(&self, vehicle_id: i64) -> Result<PresenceFacts, BusinessError> {
        let last_arrival_at = self
            .events
            .latest_of_kind(vehicle_id, PresenceEventKind::Arrival)
            .await?
            .map(|event| event.occurred_at);
        let last_departure_at = self
            .events
            .latest_of_kind(vehicle_id, PresenceEventKind::Departure)
            .await?
            .map(|event| event.occurred_at);
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
        Ok(PresenceFacts {
            last_arrival_at,
            last_departure_at,
            away,
            last_run,
        })
    }

    async fn departure_before(
        &self,
        vehicle_id: i64,
        before: NaiveDateTime,
    ) -> Result<Option<NaiveDateTime>, BusinessError> {
        Ok(self
            .events
            .latest_of_kind_before(vehicle_id, PresenceEventKind::Departure, before)
            .await?
            .map(|event| event.occurred_at))
    }
}
