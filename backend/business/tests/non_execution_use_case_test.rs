use business::domain::business_error::BusinessError;
use business::domain::enums::PresenceEventKind::{Arrival, Departure};
use business::domain::enums::{MaintenancePlanStatus, PresenceEventKind, WorkOrderOrigin, WorkOrderStatus};
use business::domain::maintenance_plan::MaintenancePlan;
use business::domain::non_execution::{
    NonExecutionKind, NonExecutionOccurrence, NonExecutionRecord, NonExecutionView,
};
use business::domain::tenant_rule_setting::{RuleSettings, TenantRuleSetting};
use business::domain::work_order::WorkOrder;
use business::gateway::non_execution_gateway::NonExecutionStore;
use business::gateway::tenant_rule_setting_gateway::TenantRuleSettingStore;
use business::use_cases::non_execution_evaluation::PresenceEvent;
use business::use_cases::non_execution_use_case::{
    ALREADY_JUSTIFIED, ALREADY_RECORDED, JUSTIFICATION_INVALID, NOT_EVIDENCED, NonExecutionUseCase,
    OCCURRENCE_NOT_FOUND, WORK_ORDER_NOT_FOUND, WORK_ORDER_NOT_OPEN,
};
use chrono::{NaiveDate, NaiveDateTime};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct World {
    plans: Vec<MaintenancePlan>,
    orders: Vec<WorkOrder>,
    presence: Vec<(i64, PresenceEvent)>,
    records: Vec<NonExecutionRecord>,
    occurrences: Vec<NonExecutionOccurrence>,
}

#[derive(Clone, Default)]
struct MemoryStore(Arc<Mutex<World>>);

impl MemoryStore {
    fn world(&self) -> std::sync::MutexGuard<'_, World> {
        self.0.lock().expect("lock")
    }
}

#[async_trait]
impl NonExecutionStore for MemoryStore {
    async fn plans_in_status(&self, statuses: &[MaintenancePlanStatus]) -> Result<Vec<MaintenancePlan>, BusinessError> {
        Ok(self.world().plans.iter().filter(|p| statuses.contains(&p.status)).cloned().collect())
    }
    async fn find_plan(&self, id: i64) -> Result<Option<MaintenancePlan>, BusinessError> {
        Ok(self.world().plans.iter().find(|p| p.id == Some(id)).cloned())
    }
    async fn save_plan(&self, plan: MaintenancePlan) -> Result<MaintenancePlan, BusinessError> {
        let mut world = self.world();
        if let Some(slot) = world.plans.iter_mut().find(|p| p.id == plan.id) {
            *slot = plan.clone();
        }
        Ok(plan)
    }
    async fn work_orders_of_plan(&self, plan_id: i64) -> Result<Vec<WorkOrder>, BusinessError> {
        Ok(self.world().orders.iter().filter(|o| o.maintenance_plan_id == Some(plan_id)).cloned().collect())
    }
    async fn find_work_order_by_uuid(&self, uuid: String) -> Result<Option<WorkOrder>, BusinessError> {
        Ok(self.world().orders.iter().find(|o| o.uuid.as_deref() == Some(uuid.as_str())).cloned())
    }
    async fn presence_events(&self, vehicle_id: i64, from: NaiveDateTime, to: NaiveDateTime) -> Result<Vec<PresenceEvent>, BusinessError> {
        let mut events: Vec<PresenceEvent> = self
            .world()
            .presence
            .iter()
            .filter(|(v, e)| *v == vehicle_id && e.at >= from && e.at <= to)
            .map(|(_, e)| *e)
            .collect();
        events.sort_by_key(|e| e.at);
        Ok(events)
    }
    async fn last_presence_before(&self, vehicle_id: i64, before: NaiveDateTime) -> Result<Option<PresenceEvent>, BusinessError> {
        Ok(self
            .world()
            .presence
            .iter()
            .filter(|(v, e)| *v == vehicle_id && e.at < before)
            .map(|(_, e)| *e)
            .max_by_key(|e| e.at))
    }
    async fn find_record_by_work_order(&self, work_order_id: i64) -> Result<Option<NonExecutionRecord>, BusinessError> {
        Ok(self.world().records.iter().find(|r| r.work_order_id == work_order_id).cloned())
    }
    async fn save_record(&self, mut record: NonExecutionRecord) -> Result<NonExecutionRecord, BusinessError> {
        let mut world = self.world();
        record.id = Some(world.records.len() as i64 + 1);
        record.uuid = Some(format!("record-{}", world.records.len() + 1));
        world.records.push(record.clone());
        Ok(record)
    }
    async fn occurrence_key_exists(&self, key: &str) -> Result<bool, BusinessError> {
        Ok(self.world().occurrences.iter().any(|o| o.business_key == key))
    }
    async fn save_occurrence(&self, mut occurrence: NonExecutionOccurrence) -> Result<NonExecutionOccurrence, BusinessError> {
        let mut world = self.world();
        match occurrence.id {
            Some(_) => {
                if let Some(slot) = world.occurrences.iter_mut().find(|o| o.id == occurrence.id) {
                    *slot = occurrence.clone();
                }
            }
            None => {
                occurrence.id = Some(world.occurrences.len() as i64 + 1);
                occurrence.uuid = Some(format!("occurrence-{}", world.occurrences.len() + 1));
                world.occurrences.push(occurrence.clone());
            }
        }
        Ok(occurrence)
    }
    async fn find_occurrence_by_uuid(&self, uuid: String) -> Result<Option<NonExecutionOccurrence>, BusinessError> {
        Ok(self.world().occurrences.iter().find(|o| o.uuid.as_deref() == Some(uuid.as_str())).cloned())
    }
    async fn history_of_work_order(&self, work_order: WorkOrder) -> Result<Option<NonExecutionView>, BusinessError> {
        let world = self.world();
        let Some(record) = world.records.iter().find(|r| Some(r.work_order_id) == work_order.id).cloned() else {
            return Ok(None);
        };
        let occurrences: Vec<_> = world.occurrences.iter().filter(|o| Some(o.record_id) == record.id).cloned().collect();
        let total_occurrences = occurrences.len() as u64;
        Ok(Some(NonExecutionView { record, work_order, occurrences, total_occurrences }))
    }
    async fn pending(&self) -> Result<Vec<NonExecutionView>, BusinessError> {
        let world = self.world();
        let mut views = Vec::new();
        for record in &world.records {
            let Some(order) = world.orders.iter().find(|o| Some(o.id) == Some(Some(record.work_order_id))) else { continue };
            if matches!(order.status, WorkOrderStatus::Concluded | WorkOrderStatus::Cancelled) {
                continue;
            }
            let all: Vec<_> = world.occurrences.iter().filter(|o| Some(o.record_id) == record.id).cloned().collect();
            let open: Vec<_> = all.iter().filter(|o| o.justified_at.is_none()).cloned().collect();
            if !open.is_empty() {
                views.push(NonExecutionView { record: record.clone(), work_order: order.clone(), occurrences: open, total_occurrences: all.len() as u64 });
            }
        }
        Ok(views)
    }
}

struct MemorySettings(RuleSettings);

#[async_trait]
impl TenantRuleSettingStore for MemorySettings {
    async fn get_current(&self, tenant_id: Option<i64>) -> Result<Option<TenantRuleSetting>, BusinessError> {
        Ok(Some(TenantRuleSetting::new(tenant_id, self.0)))
    }
    async fn save(&self, setting: TenantRuleSetting) -> Result<TenantRuleSetting, BusinessError> {
        Ok(setting)
    }
}

/// Minutes after 2026-10-10 00:00.
fn at(minutes: i64) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 10, 10).unwrap().and_hms_opt(0, 0, 0).unwrap() + chrono::Duration::minutes(minutes)
}

fn plan(id: i64, vehicle: i64) -> MaintenancePlan {
    MaintenancePlan {
        id: Some(id),
        uuid: Some(format!("plan-{id}")),
        tenant_id: Some(42),
        vehicle_id: vehicle,
        date: at(0).date(),
        planned_start: Some(at(600)),
        planned_end: Some(at(720)),
        status: MaintenancePlanStatus::Scheduled,
        affects_schedule: false,
        origin: "Manual".into(),
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn order(id: i64, vehicle: i64, plan_id: i64, status: WorkOrderStatus) -> WorkOrder {
    WorkOrder {
        id: Some(id),
        uuid: Some(format!("order-{id}")),
        tenant_id: Some(42),
        vehicle_id: vehicle,
        opened_at: None,
        odometer_km: 0.0,
        origin: WorkOrderOrigin::Manual,
        checklist_run_id: None,
        maintenance_plan_id: Some(plan_id),
        preventive_plan_id: None,
        service_type: None,
        description: "Brakes".into(),
        responsible: None,
        status,
        observation: None,
        external_service: false,
        supplier: None,
        invoice_number: None,
        invoice_value_cents: None,
        invoice_date: None,
        concluded_at: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn ev(kind: PresenceEventKind, minutes: i64) -> (i64, PresenceEvent) {
    (10, PresenceEvent { kind, at: at(minutes) })
}

fn setup(plans: Vec<MaintenancePlan>, orders: Vec<WorkOrder>, presence: Vec<(i64, PresenceEvent)>) -> (MemoryStore, NonExecutionUseCase) {
    use_case_with(plans, orders, presence, RuleSettings::default())
}

fn use_case_with(
    plans: Vec<MaintenancePlan>,
    orders: Vec<WorkOrder>,
    presence: Vec<(i64, PresenceEvent)>,
    rules: RuleSettings,
) -> (MemoryStore, NonExecutionUseCase) {
    let store = MemoryStore::default();
    *store.world() = World { plans, orders, presence, ..World::default() };
    (store.clone(), NonExecutionUseCase::new(store, MemorySettings(rules)))
}

#[tokio::test]
async fn a_visit_across_the_window_records_one_occurrence_per_unresolved_order_and_not_executes_the_plan() {
    let (store, uc) = setup(
        vec![plan(1, 10)],
        vec![order(1, 10, 1, WorkOrderStatus::Open), order(2, 10, 1, WorkOrderStatus::AwaitingParts), order(3, 10, 1, WorkOrderStatus::Concluded)],
        vec![ev(Departure, 100), ev(Arrival, 650), ev(Departure, 700)],
    );
    let outcome = uc.evaluate(at(800)).await.expect("evaluates");
    assert_eq!((outcome.occurrences_recorded, outcome.plans_concluded), (2, 0));
    let world = store.world();
    assert_eq!(world.plans[0].status, MaintenancePlanStatus::NotExecuted);
    assert_eq!(world.records.len(), 2, "one record per unresolved order");
    let first = &world.occurrences[0];
    assert_eq!(first.kind, NonExecutionKind::EnteredAndLeft);
    assert_eq!((first.dwell_minutes, first.minutes_in_window), (Some(50), Some(50)));
    assert_eq!((first.arrived_at, first.departed_at), (Some(at(650)), Some(at(700))));
    assert_eq!((first.window_start, first.window_end), (at(600), at(720)));
    assert!(first.business_key.starts_with("1:1:"));
}

#[tokio::test]
async fn evaluating_again_adds_nothing() {
    let (store, uc) = setup(vec![plan(1, 10)], vec![order(1, 10, 1, WorkOrderStatus::Open)], vec![ev(Departure, 100)]);
    assert_eq!(uc.evaluate(at(800)).await.expect("first").occurrences_recorded, 1);
    let again = uc.evaluate(at(900)).await.expect("second");
    assert_eq!(again.occurrences_recorded, 0);
    assert_eq!(store.world().occurrences.len(), 1);
    // Even forcing the plan back to Scheduled cannot duplicate: the key already exists.
    store.world().plans[0].status = MaintenancePlanStatus::Scheduled;
    assert_eq!(uc.evaluate(at(950)).await.expect("third").occurrences_recorded, 0);
    assert_eq!(store.world().occurrences.len(), 1);
}

#[tokio::test]
async fn did_not_appear_waits_for_the_tenants_grace() {
    let tight = RuleSettings { non_execution_grace_minutes: 0, ..RuleSettings::default() };
    let (_, loose) = setup(vec![plan(1, 10)], vec![order(1, 10, 1, WorkOrderStatus::Open)], vec![ev(Departure, 100)]);
    assert_eq!(loose.evaluate(at(780)).await.expect("within the default 60").occurrences_recorded, 0);
    let (store, strict) = use_case_with(vec![plan(1, 10)], vec![order(1, 10, 1, WorkOrderStatus::Open)], vec![ev(Departure, 100)], tight);
    assert_eq!(strict.evaluate(at(721)).await.expect("no grace").occurrences_recorded, 1);
    let world = store.world();
    assert_eq!(world.occurrences[0].kind, NonExecutionKind::DidNotAppear);
    assert_eq!((world.occurrences[0].dwell_minutes, world.occurrences[0].arrived_at), (None, None));
}

#[tokio::test]
async fn no_evidence_no_claim() {
    // No events at all, and present at the start: neither is an absence.
    for presence in [vec![], vec![ev(Arrival, 100)]] {
        let (store, uc) = setup(vec![plan(1, 10)], vec![order(1, 10, 1, WorkOrderStatus::Open)], presence);
        assert_eq!(uc.evaluate(at(900)).await.expect("evaluates").occurrences_recorded, 0);
        assert_eq!(store.world().plans[0].status, MaintenancePlanStatus::Scheduled);
    }
}

#[tokio::test]
async fn leaving_the_base_never_concludes_a_plan_but_resolving_every_order_does() {
    let (store, uc) = setup(
        vec![plan(1, 10)],
        vec![order(1, 10, 1, WorkOrderStatus::Concluded), order(2, 10, 1, WorkOrderStatus::Open)],
        vec![ev(Departure, 100), ev(Arrival, 650), ev(Departure, 700)],
    );
    let first = uc.evaluate(at(800)).await.expect("evaluates");
    assert_eq!(first.plans_concluded, 0);
    assert_eq!(store.world().plans[0].status, MaintenancePlanStatus::NotExecuted);
    store.world().orders[1].status = WorkOrderStatus::Concluded;
    let second = uc.evaluate(at(900)).await.expect("evaluates");
    assert_eq!(second.plans_concluded, 1);
    assert_eq!(store.world().plans[0].status, MaintenancePlanStatus::Concluded);
    assert_eq!(store.world().occurrences.len(), 1, "the history stays");
}

#[tokio::test]
async fn a_plan_without_orders_or_without_a_window_is_left_alone() {
    let mut windowless = plan(2, 10);
    windowless.planned_start = None;
    let (store, uc) = setup(vec![plan(1, 10), windowless], vec![order(5, 10, 2, WorkOrderStatus::Open)], vec![ev(Departure, 100)]);
    let outcome = uc.evaluate(at(900)).await.expect("evaluates");
    assert_eq!((outcome.plans_evaluated, outcome.occurrences_recorded), (0, 0));
    assert!(store.world().plans.iter().all(|p| p.status == MaintenancePlanStatus::Scheduled));
}

#[tokio::test]
async fn a_window_still_to_come_is_not_evaluated() {
    let (store, uc) = setup(vec![plan(1, 10)], vec![order(1, 10, 1, WorkOrderStatus::Open)], vec![ev(Departure, 100)]);
    assert_eq!(uc.evaluate(at(500)).await.expect("evaluates").occurrences_recorded, 0);
    assert_eq!(store.world().plans[0].status, MaintenancePlanStatus::Scheduled);
}

#[tokio::test]
async fn one_order_keeps_one_record_across_two_plans() {
    let mut second = plan(2, 10);
    second.planned_start = Some(at(2000));
    second.planned_end = Some(at(2120));
    let (store, uc) = setup(vec![plan(1, 10), second], vec![order(1, 10, 1, WorkOrderStatus::Open)], vec![ev(Departure, 100)]);
    uc.evaluate(at(800)).await.expect("first window");
    // The order is rescheduled into the next plan.
    store.world().orders[0].maintenance_plan_id = Some(2);
    uc.evaluate(at(2300)).await.expect("second window");
    {
        let world = store.world();
        assert_eq!(world.records.len(), 1);
        assert_eq!(world.occurrences.len(), 2);
    }
    let history = uc.history("order-1".to_string()).await.expect("history");
    assert_eq!(history.total_occurrences, 2);
}

#[tokio::test]
async fn justifying_takes_the_order_out_of_the_inbox_without_touching_its_status() {
    let (store, uc) = setup(vec![plan(1, 10)], vec![order(1, 10, 1, WorkOrderStatus::Open)], vec![ev(Departure, 100)]);
    uc.evaluate(at(800)).await.expect("evaluates");
    assert_eq!(uc.pending().await.expect("inbox").len(), 1);
    let saved = uc.justify("occurrence-1".into(), "  waiting for the part  ", "owner@x.com", at(810)).await.expect("justifies");
    assert_eq!(saved.justification.as_deref(), Some("waiting for the part"));
    assert_eq!((saved.justified_by.as_deref(), saved.justified_at), (Some("owner@x.com"), Some(at(810))));
    assert!(uc.pending().await.expect("inbox").is_empty());
    assert_eq!(store.world().orders[0].status, WorkOrderStatus::Open);
    let error = uc.justify("occurrence-1".into(), "again", "owner@x.com", at(820)).await.expect_err("twice");
    assert_eq!(error.message, ALREADY_JUSTIFIED);
    let history = uc.history("order-1".into()).await.expect("history keeps it");
    assert_eq!((history.total_occurrences, history.occurrences.len()), (1, 1));
}

#[tokio::test]
async fn a_justification_is_validated_and_the_occurrence_must_exist() {
    let (_, uc) = setup(vec![plan(1, 10)], vec![order(1, 10, 1, WorkOrderStatus::Open)], vec![ev(Departure, 100)]);
    uc.evaluate(at(800)).await.expect("evaluates");
    for bad in ["", "   ", &"x".repeat(1001)] {
        let error = uc.justify("occurrence-1".into(), bad, "a", at(810)).await.expect_err("invalid");
        assert_eq!(error.message, JUSTIFICATION_INVALID);
    }
    assert!(uc.justify("occurrence-1".into(), &"x".repeat(1000), "a", at(810)).await.is_ok());
    let missing = uc.justify("nope".into(), "text", "a", at(810)).await.expect_err("missing");
    assert_eq!(missing.message, OCCURRENCE_NOT_FOUND);
}

#[tokio::test]
async fn concluding_or_cancelling_the_order_takes_it_out_of_the_inbox() {
    for finished in [WorkOrderStatus::Concluded, WorkOrderStatus::Cancelled] {
        let (store, uc) = setup(vec![plan(1, 10)], vec![order(1, 10, 1, WorkOrderStatus::Open)], vec![ev(Departure, 100)]);
        uc.evaluate(at(800)).await.expect("evaluates");
        assert_eq!(uc.pending().await.expect("inbox").len(), 1);
        store.world().orders[0].status = finished;
        assert!(uc.pending().await.expect("inbox").is_empty());
        assert_eq!(uc.history("order-1".into()).await.expect("history").total_occurrences, 1);
    }
}

#[tokio::test]
async fn a_history_that_does_not_exist_is_not_found() {
    let (_, uc) = setup(vec![plan(1, 10)], vec![order(1, 10, 1, WorkOrderStatus::Open)], vec![]);
    assert_eq!(uc.history("order-1".into()).await.expect_err("no record").message, OCCURRENCE_NOT_FOUND);
    assert_eq!(uc.history("ghost".into()).await.expect_err("no order").message, WORK_ORDER_NOT_FOUND);
}

#[tokio::test]
async fn a_manual_record_needs_presence_evidence_and_the_tenants_minimum_dwell() {
    let visit = vec![ev(Departure, 100), ev(Arrival, 650), ev(Departure, 662)];
    let (store, uc) = setup(vec![plan(1, 10)], vec![order(1, 10, 1, WorkOrderStatus::Open)], vec![ev(Departure, 100)]);
    let none = uc.record_manual("order-1".into(), "driver said so", "owner@x.com", at(700)).await.expect_err("no evidence");
    assert_eq!(none.message, NOT_EVIDENCED);
    drop(store);

    let strict = RuleSettings { non_execution_min_dwell_minutes: 13, ..RuleSettings::default() };
    let (_, uc) = use_case_with(vec![plan(1, 10)], vec![order(1, 10, 1, WorkOrderStatus::Open)], visit.clone(), strict);
    assert_eq!(uc.record_manual("order-1".into(), "x", "a", at(700)).await.expect_err("12 < 13").message, NOT_EVIDENCED);

    let (store, uc) = setup(vec![plan(1, 10)], vec![order(1, 10, 1, WorkOrderStatus::Open)], visit);
    let saved = uc.record_manual("order-1".into(), " left early ", "owner@x.com", at(700)).await.expect("12 >= 10");
    assert_eq!(saved.kind, NonExecutionKind::Manual);
    assert_eq!((saved.dwell_minutes, saved.minutes_in_window), (Some(12), Some(12)));
    assert_eq!((saved.justification.as_deref(), saved.justified_by.as_deref()), (Some("left early"), Some("owner@x.com")));
    assert!(saved.business_key.starts_with("manual:1:1:"));
    assert_eq!(store.world().plans[0].status, MaintenancePlanStatus::NotExecuted, "the automatic pass will not repeat it");
    let twice = uc.record_manual("order-1".into(), "again", "a", at(710)).await.expect_err("same visit");
    assert_eq!(twice.message, ALREADY_RECORDED);
}

#[tokio::test]
async fn a_manual_record_refuses_a_resolved_order_an_empty_text_and_an_unknown_order() {
    let visit = vec![ev(Arrival, 650), ev(Departure, 700)];
    let (_, uc) = setup(vec![plan(1, 10)], vec![order(1, 10, 1, WorkOrderStatus::Concluded)], visit.clone());
    assert_eq!(uc.record_manual("order-1".into(), "x", "a", at(800)).await.expect_err("resolved").message, WORK_ORDER_NOT_OPEN);
    assert_eq!(uc.record_manual("order-1".into(), "", "a", at(800)).await.expect_err("empty").message, JUSTIFICATION_INVALID);
    assert_eq!(uc.record_manual("ghost".into(), "x", "a", at(800)).await.expect_err("unknown").message, WORK_ORDER_NOT_FOUND);
    // An order with no plan has no window to evidence.
    let mut loose = order(2, 10, 1, WorkOrderStatus::Open);
    loose.maintenance_plan_id = None;
    let (_, uc) = setup(vec![plan(1, 10)], vec![loose], visit);
    assert_eq!(uc.record_manual("order-2".into(), "x", "a", at(800)).await.expect_err("no plan").message, NOT_EVIDENCED);
}
