use business::domain::business_error::BusinessError;
use business::domain::enums::{
    GarageAttendanceOrigin, GarageAttendanceStatus, GarageServiceGroup, PresenceEventKind,
    TripStatus,
};
use business::domain::extra_trip::ExtraTrip;
use business::domain::garage_attendance::GarageAttendance;
use business::domain::garage_service::GarageService;
use business::domain::garage_service_model::{GarageServiceApplicability, GarageServiceModel};
use business::domain::tenant_rule_setting::TenantRuleSetting;
use business::domain::vehicle_presence_event::VehiclePresenceEvent;
use business::gateway::customer_day_off_gateway::CustomerDayOffGateway;
use business::gateway::daily_schedule_gateway::DailyScheduleGateway;
use business::gateway::effective_schedule_gateway::EffectiveScheduleDatabaseGateway;
use business::gateway::effective_schedule_gateway::EffectiveScheduleStore;
use business::gateway::extra_trip_gateway::ExtraTripGateway;
use business::gateway::extra_trip_gateway::GarageValidityTripStore;
use business::gateway::fuel_entry_gateway::FuelEntryGateway;
use business::gateway::fuel_gauge_setting_gateway::FuelGaugeSettingGateway;
use business::gateway::garage_auto_departure_gateway::GarageAutoDepartureDatabaseGateway;
use business::gateway::garage_auto_departure_gateway::GarageAutoDepartureStore;
use business::gateway::garage_service_model_gateway::GarageServiceModelGateway;
use business::gateway::garage_service_model_gateway::GarageServiceModelStore;
use business::gateway::holiday_gateway::HolidayGateway;
use business::gateway::km_evolution_gateway::KmEvolutionGateway;
use business::gateway::schedule_exception_gateway::ScheduleExceptionGateway;
use business::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway;
use business::gateway::tenant_rule_setting_gateway::TenantRuleSettingStore;
use business::gateway::transport_demand_allocation_gateway::TransportDemandAllocationGateway;
use business::gateway::transport_demand_gateway::TransportDemandGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::gateway::vehicle_presence_event_gateway::GarageValidityPresenceStore;
use business::gateway::vehicle_presence_event_gateway::VehiclePresenceEventGateway;
use business::use_cases::effective_schedule::ItemKind;
use business::use_cases::effective_schedule_use_case::{Departure, EffectiveScheduleUseCase};
use business::use_cases::fuel_gauge_use_case::{
    FuelGaugeReader, FuelGaugeUseCase, GaugeUnavailable, TankGauge,
};
use business::use_cases::garage_auto_departure_use_case::{
    GarageAutoDepartureUseCase, automatic_departure_justification, due_departure,
};
use business::use_cases::garage_validity_use_case::GarageValidityUseCase;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use entity::audit::{AuditUser, run_with_user};
use entity::{
    customer_day_off_entity, daily_schedule_entity, extra_trip_entity, garage_attendance_entity,
    holiday_entity, schedule_exception_entity, transport_demand_allocation_entity,
    transport_demand_entity,
};
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{DatabaseBackend, MockDatabase};
use std::sync::{Arc, Mutex};

fn at(hour: u32, minute: u32) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(
        &format!("2026-10-14 {hour:02}:{minute:02}:00"),
        "%Y-%m-%d %H:%M:%S",
    )
    .expect("valid test time")
}

fn attendance(checked_in_at: NaiveDateTime) -> GarageAttendance {
    GarageAttendance {
        id: Some(1),
        uuid: Some("attendance-1".into()),
        tenant_id: Some(7),
        vehicle_id: 11,
        attendance_date: checked_in_at.date(),
        checked_in_at,
        status: GarageAttendanceStatus::Open,
        manual_priority: Some(2),
        released_at: None,
        automatic_departure_justification: None,
        origin: GarageAttendanceOrigin::Manual,
        active_marker: Some(1),
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn departure(at: NaiveDateTime, kind: ItemKind, name: &str) -> Departure {
    Departure {
        at,
        is_trip: matches!(kind, ItemKind::OneOffTrip | ItemKind::ExtraTrip),
        kind,
        trip_id: None,
        name: Some(name.to_string()),
    }
}

#[tokio::test]
async fn run_returns_zero_when_the_schedule_has_no_active_triages() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([Vec::<transport_demand_entity::Model>::new()])
        .append_query_results([Vec::<transport_demand_allocation_entity::Model>::new()])
        .append_query_results([Vec::<daily_schedule_entity::Model>::new()])
        .append_query_results([Vec::<schedule_exception_entity::Model>::new()])
        .append_query_results([Vec::<extra_trip_entity::Model>::new()])
        .append_query_results([Vec::<holiday_entity::Model>::new()])
        .append_query_results([Vec::<customer_day_off_entity::Model>::new()])
        .append_query_results([Vec::<garage_attendance_entity::Model>::new()])
        .into_connection();
    let schedule = EffectiveScheduleUseCase::new(EffectiveScheduleDatabaseGateway::new(
        TransportDemandGateway::new(db.clone()),
        TransportDemandAllocationGateway::new(db.clone()),
        DailyScheduleGateway::new(db.clone()),
        ScheduleExceptionGateway::new(db.clone()),
        HolidayGateway::new(db.clone()),
        CustomerDayOffGateway::new(db.clone()),
        ExtraTripGateway::new(db.clone()),
    ));
    let rules = TenantRuleSettingGateway::new(db.clone());
    let validity = GarageValidityUseCase::new(
        VehiclePresenceEventGateway::new(db.clone()),
        ExtraTripGateway::new(db.clone()),
        GarageServiceModelGateway::new(db.clone()),
        TenantRuleSettingGateway::new(db.clone()),
        FuelGaugeUseCase::new(
            FuelEntryGateway::new(db.clone()),
            VehicleGateway::new(db.clone()),
            KmEvolutionGateway::new(db.clone()),
            FuelGaugeSettingGateway::new(db.clone()),
            rules,
        ),
    );
    let use_case = GarageAutoDepartureUseCase::new(
        GarageAutoDepartureDatabaseGateway::new(db),
        schedule,
        validity,
    );
    let owner = AuditUser {
        id: 1,
        email: "owner@example.com".into(),
        tenant_id: Some(42),
        enforce_tenant: true,
    };

    let result = run_with_user(Some(owner), use_case.run()).await.unwrap();

    assert_eq!(result.closed, 0);
}

#[derive(Default)]
struct TripScheduleStore;

#[async_trait]
impl EffectiveScheduleStore for TripScheduleStore {
    async fn demands(
        &self,
    ) -> Result<Vec<business::domain::transport_demand::TransportDemand>, BusinessError> {
        Ok(Vec::new())
    }

    async fn allocations(
        &self,
    ) -> Result<
        Vec<business::domain::transport_demand_allocation::TransportDemandAllocation>,
        BusinessError,
    > {
        Ok(Vec::new())
    }

    async fn daily_entries(
        &self,
        _date: NaiveDate,
    ) -> Result<Vec<business::domain::daily_schedule::DailySchedule>, BusinessError> {
        Ok(Vec::new())
    }

    async fn exceptions(
        &self,
        _date: NaiveDate,
    ) -> Result<Vec<business::domain::schedule_exception::ScheduleException>, BusinessError> {
        Ok(Vec::new())
    }

    async fn trips(&self, date: NaiveDate) -> Result<Vec<ExtraTrip>, BusinessError> {
        Ok(vec![ExtraTrip {
            id: Some(5),
            uuid: Some("trip-5".into()),
            tenant_id: Some(42),
            order_code: "TRIP-5".into(),
            trip_date: date,
            customer_id: None,
            start_time: Some(NaiveTime::MIN),
            return_date: None,
            return_time: None,
            destination: None,
            origin_city: None,
            stops: None,
            preferred_vehicle_type: None,
            driver_id: None,
            second_driver_id: None,
            vehicle_id: Some(10),
            freight_value_cents: None,
            payment: None,
            status: TripStatus::Scheduled,
            origin: None,
            import_batch_id: None,
            imported_at: None,
            replaced_by_id: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }])
    }

    async fn is_holiday(&self, _date: NaiveDate) -> Result<bool, BusinessError> {
        Ok(false)
    }

    async fn day_off_customers(&self, _date: NaiveDate) -> Result<Vec<i64>, BusinessError> {
        Ok(Vec::new())
    }
}

type ClosedAttendance = (GarageAttendance, Vec<GarageService>);

#[derive(Clone)]
struct ActiveAttendanceStore {
    closed: Arc<Mutex<Vec<ClosedAttendance>>>,
}

#[async_trait]
impl GarageAutoDepartureStore for ActiveAttendanceStore {
    async fn find_active_with_services(
        &self,
    ) -> Result<Vec<(GarageAttendance, Vec<GarageService>)>, BusinessError> {
        let check_in = chrono::Utc::now().naive_utc() - chrono::Duration::days(1);
        let mut triage = attendance(check_in);
        triage.vehicle_id = 10;
        Ok(vec![(
            triage,
            vec![GarageService {
                id: Some(22),
                uuid: Some("service-22".into()),
                tenant_id: Some(42),
                attendance_id: 1,
                service_model_id: 7,
                name_key: "brakes".into(),
                state: business::domain::enums::GarageServiceState::Pending,
                performed_at: None,
                marked_at: None,
                forced_pending_at: None,
                created_at: None,
                created_by: None,
                updated_at: None,
                updated_by: None,
            }],
        )])
    }

    async fn close_automatically(
        &self,
        attendance: GarageAttendance,
        services: Vec<GarageService>,
    ) -> Result<bool, BusinessError> {
        self.closed.lock().unwrap().push((attendance, services));
        Ok(true)
    }
}

#[derive(Default)]
struct EmptyPresenceStore;

#[async_trait]
impl GarageValidityPresenceStore for EmptyPresenceStore {
    async fn latest_of_kind(
        &self,
        _vehicle_id: i64,
        _kind: PresenceEventKind,
    ) -> Result<Option<VehiclePresenceEvent>, BusinessError> {
        Ok(None)
    }

    async fn latest_of_kind_before(
        &self,
        _vehicle_id: i64,
        _kind: PresenceEventKind,
        _before: NaiveDateTime,
    ) -> Result<Option<VehiclePresenceEvent>, BusinessError> {
        Ok(None)
    }
}

#[derive(Default)]
struct EmptyUpcomingTrips;

#[async_trait]
impl GarageValidityTripStore for EmptyUpcomingTrips {
    async fn upcoming_for_vehicle(
        &self,
        _vehicle_id: i64,
        _from: NaiveDate,
    ) -> Result<Vec<ExtraTrip>, BusinessError> {
        Ok(Vec::new())
    }
}

struct OneRequiredService;

#[async_trait]
impl GarageServiceModelStore for OneRequiredService {
    async fn save(&self, model: GarageServiceModel) -> Result<GarageServiceModel, BusinessError> {
        Ok(model)
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<GarageServiceModel>, BusinessError> {
        Ok((id == 7).then(|| GarageServiceModel {
            id: Some(7),
            uuid: Some("model-7".into()),
            tenant_id: Some(42),
            name: "Brake inspection".into(),
            name_key: "brakes".into(),
            display_order: 1,
            active: true,
            service_group: GarageServiceGroup::External,
            required_for_departure: true,
            governed_by_tank: false,
            applicability: GarageServiceApplicability::All,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }))
    }

    async fn get_by_uuid(
        &self,
        _uuid: String,
    ) -> Result<Option<GarageServiceModel>, BusinessError> {
        Ok(None)
    }

    async fn find_by_key(
        &self,
        _name_key: &str,
        _tenant_id: Option<i64>,
    ) -> Result<Option<GarageServiceModel>, BusinessError> {
        Ok(None)
    }

    async fn list_page(
        &self,
        _page: u64,
        _page_size: u64,
    ) -> Result<(Vec<GarageServiceModel>, u64), BusinessError> {
        Ok((Vec::new(), 0))
    }
}

#[derive(Default)]
struct EmptyTenantRules;

#[async_trait]
impl TenantRuleSettingStore for EmptyTenantRules {
    async fn get_current(
        &self,
        _tenant_id: Option<i64>,
    ) -> Result<Option<TenantRuleSetting>, BusinessError> {
        Ok(None)
    }

    async fn save(&self, setting: TenantRuleSetting) -> Result<TenantRuleSetting, BusinessError> {
        Ok(setting)
    }
}

#[derive(Default)]
struct UnavailableGauge;

#[async_trait]
impl FuelGaugeReader for UnavailableGauge {
    async fn read_gauge(&self, _vehicle_id: i64) -> Result<TankGauge, BusinessError> {
        Ok(TankGauge::Unavailable(GaugeUnavailable::NoTankRegistered))
    }
}

#[tokio::test]
async fn run_closes_a_triage_for_a_due_trip_and_marks_required_pending_services() {
    let store = ActiveAttendanceStore {
        closed: Arc::new(Mutex::new(Vec::new())),
    };
    let closed = Arc::clone(&store.closed);
    let validity = GarageValidityUseCase::new(
        EmptyPresenceStore,
        EmptyUpcomingTrips,
        OneRequiredService,
        EmptyTenantRules,
        UnavailableGauge,
    );
    let schedule = EffectiveScheduleUseCase::new(TripScheduleStore);
    let now = chrono::Utc::now().naive_utc();
    let departures = schedule
        .departures_by_vehicle(now.date(), now.date())
        .await
        .expect("trip is present in the schedule");
    assert_eq!(departures.get(&10).map(Vec::len), Some(1));
    let use_case = GarageAutoDepartureUseCase::new(store, schedule, validity);

    let outcome = use_case.run().await.expect("due attendance is closed");
    assert_eq!(outcome.closed, 1);
    let closed = closed.lock().unwrap();
    assert_eq!(closed.len(), 1);
    assert_eq!(closed[0].0.status, GarageAttendanceStatus::LeftForOperation);
    assert!(closed[0].0.released_at.is_some());
    assert!(
        closed[0]
            .0
            .automatic_departure_justification
            .as_deref()
            .is_some_and(|text| text.contains("TRIP-5") && text.contains("Brake inspection"))
    );
    assert_eq!(closed[0].1.len(), 1);
    assert_eq!(
        closed[0].1[0].state,
        business::domain::enums::GarageServiceState::NotDone
    );
}

#[test]
fn only_due_trip_or_charter_starts_after_check_in() {
    let triage = attendance(at(8, 0));
    let departures = vec![
        departure(at(7, 30), ItemKind::OneOffTrip, "already underway"),
        departure(at(9, 0), ItemKind::Line, "recurring line"),
        departure(at(10, 0), ItemKind::ExtraTrip, "order 42"),
        departure(at(11, 0), ItemKind::ExtraLine, "charter 19"),
    ];

    assert!(due_departure(&triage, &departures, at(9, 59)).is_none());
    assert_eq!(
        due_departure(&triage, &departures, at(12, 0)).map(|item| item.name.as_deref()),
        Some(Some("order 42"))
    );
}

#[test]
fn a_departure_window_open_at_check_in_does_not_close_the_triage() {
    let triage = attendance(at(8, 0));
    let departures = vec![
        departure(at(7, 30), ItemKind::OneOffTrip, "started earlier"),
        departure(at(8, 0), ItemKind::ExtraLine, "starts at check in"),
        departure(at(9, 0), ItemKind::Line, "recurring line"),
    ];

    assert!(due_departure(&triage, &departures, at(12, 0)).is_none());
}

#[test]
fn justification_names_the_departure_type_time_and_unperformed_services() {
    let scheduled = departure(at(10, 15), ItemKind::ExtraLine, "charter 19");
    let reason = automatic_departure_justification(
        &scheduled,
        &["External wash".into(), "Interior cleaning".into()],
    );

    assert!(reason.contains("fretamento 'charter 19'"));
    assert!(reason.contains("2026-10-14 10:15"));
    assert!(reason.contains("External wash, Interior cleaning"));
}
