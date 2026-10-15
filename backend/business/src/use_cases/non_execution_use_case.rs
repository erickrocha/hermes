use crate::domain::business_error::BusinessError;
use crate::domain::enums::MaintenancePlanStatus;
use crate::domain::maintenance_plan::MaintenancePlan;
use crate::domain::non_execution::{
    EvaluationOutcome, NonExecutionKind, NonExecutionOccurrence, NonExecutionRecord, NonExecutionView,
};
use crate::domain::tenant_rule_setting::RuleSettings;
use crate::domain::work_order::WorkOrder;
use crate::gateway::non_execution_gateway::NonExecutionStore;
use crate::gateway::tenant_rule_setting_gateway::TenantRuleSettingStore;
use crate::use_cases::non_execution_evaluation::{
    Decision, Evidence, PresenceEvent, all_resolved, decide, did_not_appear_key, entered_and_left_key,
    is_resolved, manual_evidence, manual_key,
};
use chrono::{Duration, NaiveDateTime};
use std::collections::HashMap;

pub const WORK_ORDER_NOT_FOUND: &str = "Work order not found";
pub const OCCURRENCE_NOT_FOUND: &str = "Non-execution occurrence or record not found";
pub const ALREADY_JUSTIFIED: &str = "This non-execution occurrence is already justified";
pub const JUSTIFICATION_INVALID: &str = "The justification must have between 1 and 1000 characters";
pub const WORK_ORDER_NOT_OPEN: &str = "Only an unresolved work order can have a manual non-execution";
pub const NOT_EVIDENCED: &str = "The tracker does not evidence this non-execution";
pub const ALREADY_RECORDED: &str = "This non-execution was already recorded";

const JUSTIFICATION_MAX: usize = 1000;
/// `design.md`: presence is read from this long before the window opens.
const LOOKBACK_HOURS: i64 = 24;

/// `EPIC-MT-04` (`C-026`): a planned maintenance whose work orders were not resolved -- the
/// vehicle came and left, or never came. See `non_execution_evaluation` for the pure rules.
pub struct NonExecutionUseCase {
    store: Box<dyn NonExecutionStore>,
    settings: Box<dyn TenantRuleSettingStore>,
}

impl NonExecutionUseCase {
    pub fn new<S, T>(store: S, settings: T) -> Self
    where
        S: NonExecutionStore + 'static,
        T: TenantRuleSettingStore + 'static,
    {
        Self { store: Box::new(store), settings: Box::new(settings) }
    }

    /// `SR-MT-04-12`: one idempotent pass over the caller's visible plans. It only ever adds.
    pub async fn evaluate(&self, now: NaiveDateTime) -> Result<EvaluationOutcome, BusinessError> {
        log::info!("[NonExecutionUseCase] evaluating planned maintenance at {now}");
        let mut outcome = EvaluationOutcome::default();
        let mut rules: HashMap<Option<i64>, RuleSettings> = HashMap::new();
        let plans = self
            .store
            .plans_in_status(&[MaintenancePlanStatus::Scheduled, MaintenancePlanStatus::NotExecuted])
            .await?;
        for plan in plans {
            let (Some(plan_id), Some(start), Some(end)) = (plan.id, plan.planned_start, plan.planned_end)
            else {
                continue;
            };
            let orders = self.store.work_orders_of_plan(plan_id).await?;
            if orders.is_empty() {
                continue;
            }
            outcome.plans_evaluated += 1;
            let statuses: Vec<_> = orders.iter().map(|o| o.status.clone()).collect();
            if all_resolved(&statuses) {
                self.store.save_plan(MaintenancePlan { status: MaintenancePlanStatus::Concluded, ..plan }).await?;
                outcome.plans_concluded += 1;
                continue;
            }
            if plan.status != MaintenancePlanStatus::Scheduled || now <= start {
                continue;
            }
            let grace = match rules.get(&plan.tenant_id) {
                Some(known) => known.non_execution_grace_minutes,
                None => {
                    let fresh = self.rules_of(plan.tenant_id).await?;
                    rules.insert(plan.tenant_id, fresh);
                    fresh.non_execution_grace_minutes
                }
            };
            let events = self.presence(plan.vehicle_id, start, now).await?;
            let (kind, evidence) = match decide(&events, start, end, now, grace) {
                Decision::Nothing => continue,
                Decision::EnteredAndLeft(evidence) => (NonExecutionKind::EnteredAndLeft, Some(evidence)),
                Decision::DidNotAppear => (NonExecutionKind::DidNotAppear, None),
            };
            for order in orders.iter().filter(|o| !is_resolved(&o.status)) {
                let Some(order_id) = order.id else { continue };
                let key = match &evidence {
                    Some(e) => entered_and_left_key(plan_id, order_id, e),
                    None => did_not_appear_key(plan_id, order_id, start),
                };
                if self.add_occurrence(&plan, order, kind, key, evidence, None, now).await? {
                    outcome.occurrences_recorded += 1;
                }
            }
            self.store.save_plan(MaintenancePlan { status: MaintenancePlanStatus::NotExecuted, ..plan }).await?;
        }
        Ok(outcome)
    }

    /// `SR-MT-04-08/09`: the inbox rows, ordered by vehicle then work order.
    pub async fn pending(&self) -> Result<Vec<NonExecutionView>, BusinessError> {
        self.store.pending().await
    }

    /// The whole history of an order, justified occurrences included.
    pub async fn history(&self, work_order_uuid: String) -> Result<NonExecutionView, BusinessError> {
        let work_order = self.work_order(work_order_uuid).await?;
        self.store
            .history_of_work_order(work_order)
            .await?
            .ok_or_else(|| BusinessError::new(OCCURRENCE_NOT_FOUND.to_string()))
    }

    /// `SR-MT-04-09`: records who justified and when; never touches the order's status.
    pub async fn justify(
        &self,
        occurrence_uuid: String,
        justification: &str,
        by: &str,
        now: NaiveDateTime,
    ) -> Result<NonExecutionOccurrence, BusinessError> {
        let occurrence = self
            .store
            .find_occurrence_by_uuid(occurrence_uuid)
            .await?
            .ok_or_else(|| BusinessError::new(OCCURRENCE_NOT_FOUND.to_string()))?;
        let text = valid_justification(justification)?;
        if occurrence.justified_at.is_some() {
            return Err(BusinessError::new(ALREADY_JUSTIFIED.to_string()));
        }
        log::info!("[NonExecutionUseCase] justifying a non-execution occurrence");
        self.store
            .save_occurrence(NonExecutionOccurrence {
                justification: Some(text),
                justified_by: Some(by.to_string()),
                justified_at: Some(now),
                ..occurrence
            })
            .await
    }

    /// `SR-MT-04-10`: only with presence evidence; recorded already justified, since the manager's
    /// reason is mandatory, and the plan leaves `Scheduled` so the automatic pass does not repeat it.
    pub async fn record_manual(
        &self,
        work_order_uuid: String,
        justification: &str,
        by: &str,
        now: NaiveDateTime,
    ) -> Result<NonExecutionOccurrence, BusinessError> {
        let text = valid_justification(justification)?;
        let order = self.work_order(work_order_uuid).await?;
        if is_resolved(&order.status) {
            return Err(BusinessError::new(WORK_ORDER_NOT_OPEN.to_string()));
        }
        let not_evidenced = || BusinessError::new(NOT_EVIDENCED.to_string());
        let plan = match order.maintenance_plan_id {
            Some(id) => self.store.find_plan(id).await?.ok_or_else(not_evidenced)?,
            None => return Err(not_evidenced()),
        };
        let (Some(plan_id), Some(order_id), Some(start), Some(end)) =
            (plan.id, order.id, plan.planned_start, plan.planned_end)
        else {
            return Err(not_evidenced());
        };
        let min_dwell = self.rules_of(plan.tenant_id).await?.non_execution_min_dwell_minutes;
        let events = self.presence(plan.vehicle_id, start, now).await?;
        let evidence = manual_evidence(&events, start, end, min_dwell).ok_or_else(not_evidenced)?;
        log::info!("[NonExecutionUseCase] recording a manual non-execution for work order {order_id}");
        let key = manual_key(plan_id, order_id, &evidence);
        let justified = Some((text, by.to_string()));
        let saved = self
            .add_occurrence_returning(&plan, &order, NonExecutionKind::Manual, key, Some(evidence), justified, now)
            .await?
            .ok_or_else(|| BusinessError::new(ALREADY_RECORDED.to_string()))?;
        if plan.status == MaintenancePlanStatus::Scheduled {
            self.store.save_plan(MaintenancePlan { status: MaintenancePlanStatus::NotExecuted, ..plan }).await?;
        }
        Ok(saved)
    }

    async fn work_order(&self, uuid: String) -> Result<WorkOrder, BusinessError> {
        self.store
            .find_work_order_by_uuid(uuid)
            .await?
            .ok_or_else(|| BusinessError::new(WORK_ORDER_NOT_FOUND.to_string()))
    }

    async fn rules_of(&self, tenant_id: Option<i64>) -> Result<RuleSettings, BusinessError> {
        Ok(self.settings.get_current(tenant_id).await?.map(|s| s.settings()).unwrap_or_default())
    }

    /// The events the decision needs: the lookback period plus the last event before the window,
    /// however old, which is the state the window opened in.
    async fn presence(
        &self,
        vehicle_id: i64,
        start: NaiveDateTime,
        now: NaiveDateTime,
    ) -> Result<Vec<PresenceEvent>, BusinessError> {
        let from = start - Duration::hours(LOOKBACK_HOURS);
        let mut events = self.store.presence_events(vehicle_id, from, now).await?;
        if let Some(anchor) = self.store.last_presence_before(vehicle_id, start).await?
            && anchor.at < from
        {
            events.insert(0, anchor);
        }
        Ok(events)
    }

    #[allow(clippy::too_many_arguments)]
    async fn add_occurrence(
        &self,
        plan: &MaintenancePlan,
        order: &WorkOrder,
        kind: NonExecutionKind,
        key: String,
        evidence: Option<Evidence>,
        justified: Option<(String, String)>,
        now: NaiveDateTime,
    ) -> Result<bool, BusinessError> {
        Ok(self
            .add_occurrence_returning(plan, order, kind, key, evidence, justified, now)
            .await?
            .is_some())
    }

    /// `SR-MT-04-06/07`: the order's record is created on its first occurrence; a key that already
    /// exists adds nothing (`None`).
    #[allow(clippy::too_many_arguments)]
    async fn add_occurrence_returning(
        &self,
        plan: &MaintenancePlan,
        order: &WorkOrder,
        kind: NonExecutionKind,
        key: String,
        evidence: Option<Evidence>,
        justified: Option<(String, String)>,
        now: NaiveDateTime,
    ) -> Result<Option<NonExecutionOccurrence>, BusinessError> {
        let (Some(plan_id), Some(order_id), Some(start), Some(end)) =
            (plan.id, order.id, plan.planned_start, plan.planned_end)
        else {
            return Ok(None);
        };
        if self.store.occurrence_key_exists(&key).await? {
            return Ok(None);
        }
        let record = match self.store.find_record_by_work_order(order_id).await? {
            Some(record) => record,
            None => {
                self.store
                    .save_record(NonExecutionRecord {
                        id: None,
                        uuid: None,
                        tenant_id: plan.tenant_id,
                        work_order_id: order_id,
                        vehicle_id: order.vehicle_id,
                        created_at: None,
                        created_by: None,
                        updated_at: None,
                        updated_by: None,
                    })
                    .await?
            }
        };
        let Some(record_id) = record.id else {
            return Ok(None);
        };
        let (justification, justified_by, justified_at) = match justified {
            Some((text, by)) => (Some(text), Some(by), Some(now)),
            None => (None, None, None),
        };
        let saved = self
            .store
            .save_occurrence(NonExecutionOccurrence {
                id: None,
                uuid: None,
                tenant_id: plan.tenant_id,
                record_id,
                maintenance_plan_id: plan_id,
                kind,
                business_key: key,
                window_start: start,
                window_end: end,
                arrived_at: evidence.and_then(|e| e.arrived_at),
                departed_at: evidence.map(|e| e.departed_at),
                dwell_minutes: evidence.and_then(|e| e.dwell_minutes),
                minutes_in_window: evidence.map(|e| e.minutes_in_window),
                justification,
                justified_by,
                justified_at,
                created_at: None,
                created_by: None,
                updated_at: None,
                updated_by: None,
            })
            .await?;
        Ok(Some(saved))
    }
}

fn valid_justification(text: &str) -> Result<String, BusinessError> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > JUSTIFICATION_MAX {
        return Err(BusinessError::new(JUSTIFICATION_INVALID.to_string()));
    }
    Ok(text.to_string())
}
