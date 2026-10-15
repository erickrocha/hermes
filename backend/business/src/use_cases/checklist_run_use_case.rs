use crate::domain::business_error::BusinessError;
use crate::domain::checklist_answer::ChecklistAnswer;
use crate::domain::checklist_run::ChecklistRun;
use crate::domain::enums::{
    AnswerStatus, ChecklistType, KmOrigin, Role, WorkOrderItemStatus, WorkOrderOrigin,
    WorkOrderStatus,
};
use crate::domain::km_evolution::KmEvolution;
use crate::domain::work_order::WorkOrder;
use crate::domain::work_order_item::WorkOrderItem;
use crate::gateway::checklist_run_gateway::ChecklistRunStore;
use crate::use_cases::km_evolution_use_case::KmEvolutionUseCase;
use std::collections::{HashMap, HashSet};

pub const NOT_A_TENANT_TEMPLATE: &str =
    "The checklist template named does not belong to this tenant";
pub const NOT_A_TENANT_VEHICLE: &str = "The vehicle named does not belong to this tenant";
pub const NOT_A_DRIVER: &str = "The person named is not an active driver of this tenant";
/// `TRM-105`: a driver may hold at most one open vehicle at a time.
pub const DRIVER_ALREADY_HOLDS_A_VEHICLE: &str =
    "This driver already holds an open checklist on another vehicle";
pub const RETURN_REQUIRES_OPENING_CHECKLIST: &str =
    "A return checklist must name the departure it closes";
pub const MAY_NOT_NAME_AN_OPENING_CHECKLIST: &str =
    "Only a return checklist may name the departure it closes";
pub const OPENING_CHECKLIST_NOT_FOUND: &str = "The departure checklist named does not exist";
pub const OPENING_CHECKLIST_WRONG_VEHICLE: &str =
    "The departure checklist named belongs to a different vehicle";
pub const OPENING_CHECKLIST_ALREADY_CLOSED: &str =
    "The departure checklist named has already been closed by a return";
/// `TRM-101`: a checklist that carries no answered items is refused.
pub const AT_LEAST_ONE_ANSWER_REQUIRED: &str = "A checklist needs at least one answered item";
pub const DUPLICATE_ANSWER_FOR_ITEM: &str = "An item may be answered at most once per checklist";
pub const ANSWER_NOT_A_TEMPLATE_ITEM: &str =
    "An answered item does not belong to this checklist's own template";

/// `SR-CA-03-08` (`C-031`): the official reading never moves backwards by a
/// checklist; a wrong official reading is corrected through the manual
/// kilometre record (`HRMS-650`), not by submitting a lower one.
pub const ODOMETER_NEGATIVE: &str = "The odometer reading cannot be negative";
pub const ODOMETER_BELOW_OFFICIAL: &str =
    "The odometer reading is lower than the vehicle's official reading";

pub struct ChecklistRunUseCase {
    store: Box<dyn ChecklistRunStore>,
    km_evolution: KmEvolutionUseCase,
}

impl ChecklistRunUseCase {
    pub fn new<S: ChecklistRunStore + 'static>(store: S, km_evolution: KmEvolutionUseCase) -> Self {
        Self {
            store: Box::new(store),
            km_evolution,
        }
    }

    /// `HRMS-652`/`HRMS-654`: opening (`Departure`), closing (`Return`) and
    /// a one-off (`Standalone`) check are all this one operation -- the
    /// difference is entirely in `validated`'s branch on
    /// `run.checklist_type`. The run and its answers are written in one
    /// transaction (`PD-027`-style all-or-nothing, `ChecklistTemplateUseCase
    /// ::create`'s own technique); every accepted run also writes the
    /// vehicle's official odometer entry (`EPIC-CK-01-S01`, `AD-041`), never
    /// a second, parallel field.
    pub async fn create(
        &self,
        run: ChecklistRun,
        answers: Vec<ChecklistAnswer>,
    ) -> Result<(ChecklistRun, Vec<ChecklistAnswer>), BusinessError> {
        let run = self.validated(run).await?;
        let (answers, item_info) = self.validated_answers(&run, answers).await?;

        let (saved_run, mut saved_answers) =
            self.store.create_run_with_answers(run, answers).await?;

        let reading = KmEvolution {
            id: None,
            uuid: None,
            tenant_id: saved_run.tenant_id,
            vehicle_id: saved_run.vehicle_id,
            km: saved_run.odometer_km,
            recorded_at: saved_run.created_at,
            origin: KmOrigin::DriverChecklist,
            source_entity: Some("checklist_run".to_string()),
            source_entity_id: saved_run.id,
            notes: None,
            recorded_by_user_id: Some(saved_run.driver_id),
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        };
        self.km_evolution.create(reading).await?;

        let flagged: Vec<ChecklistAnswer> = saved_answers
            .iter()
            .filter(|a| a.status == AnswerStatus::NonConforming)
            .filter(|a| {
                item_info
                    .get(&a.checklist_template_item_id)
                    .is_some_and(|(_, generates_work_order)| *generates_work_order)
            })
            .cloned()
            .collect();

        if !flagged.is_empty() {
            let work_order_id = self
                .open_work_order_for_flagged_answers(&saved_run, &flagged, &item_info)
                .await?;
            let flagged_item_ids: HashSet<i64> = flagged
                .iter()
                .map(|a| a.checklist_template_item_id)
                .collect();
            for answer in saved_answers.iter_mut() {
                if flagged_item_ids.contains(&answer.checklist_template_item_id) {
                    answer.work_order_id = Some(work_order_id);
                    *answer = self.store.save_answer(answer.clone()).await?;
                }
            }
        }

        Ok((saved_run, saved_answers))
    }

    /// `TRM-101`: at least one answered item, each naming an item of the
    /// run's own template, and each item answered at most once. Also
    /// collects each item's own description and "per-item OS flag"
    /// (`generates_work_order`), keyed by `checklist_template_item_id` --
    /// `create` needs both after saving to decide which answers open a
    /// work order (`EPIC-CK-03-S02`).
    async fn validated_answers(
        &self,
        run: &ChecklistRun,
        answers: Vec<ChecklistAnswer>,
    ) -> Result<(Vec<ChecklistAnswer>, HashMap<i64, (String, bool)>), BusinessError> {
        if answers.is_empty() {
            return Err(BusinessError::new(AT_LEAST_ONE_ANSWER_REQUIRED.to_string()));
        }

        let mut seen = HashSet::new();
        let mut item_info = HashMap::new();
        for answer in &answers {
            if !seen.insert(answer.checklist_template_item_id) {
                return Err(BusinessError::new(DUPLICATE_ANSWER_FOR_ITEM.to_string()));
            }
            let item = self
                .store
                .find_template_item(answer.checklist_template_item_id)
                .await?;
            let Some(item) = item.filter(|i| {
                i.checklist_template_id == run.checklist_template_id && i.tenant_id == run.tenant_id
            }) else {
                return Err(BusinessError::new(ANSWER_NOT_A_TEMPLATE_ITEM.to_string()));
            };
            item_info.insert(
                answer.checklist_template_item_id,
                (item.description, item.generates_work_order),
            );
        }

        Ok((answers, item_info))
    }

    /// `EPIC-CK-03-S02` (`HRMS-653`, `TRM-115`/`TRM-116`): exactly one work
    /// order per checklist run, never one per flagged item, opened for
    /// every non-conforming answer whose own template item carries the
    /// "per-item OS flag." Persisted directly through the gateways, not
    /// `WorkOrderUseCase::create` -- that would also write a second
    /// `km_evolution` entry for the same physical reading the checklist's
    /// own submission already recorded (`TRM-121`), the exact second-writer
    /// shape `AD-041` exists to prevent, just relocated instead of removed.
    /// `service_type` is left blank for the manager to classify (`TRM-117`).
    async fn open_work_order_for_flagged_answers(
        &self,
        run: &ChecklistRun,
        flagged: &[ChecklistAnswer],
        item_info: &HashMap<i64, (String, bool)>,
    ) -> Result<i64, BusinessError> {
        let work_order = WorkOrder {
            id: None,
            uuid: None,
            tenant_id: run.tenant_id,
            vehicle_id: run.vehicle_id,
            opened_at: run.created_at,
            odometer_km: run.odometer_km,
            origin: WorkOrderOrigin::Checklist,
            checklist_run_id: run.id,
            maintenance_plan_id: None,
            preventive_plan_id: None,
            service_type: None,
            description: "Opened automatically from a driver checklist's flagged items".to_string(),
            responsible: None,
            status: WorkOrderStatus::Open,
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
        };
        let work_order_id = self
            .store
            .save_work_order(work_order)
            .await?
            .id
            .unwrap_or_default();

        for answer in flagged {
            let (description, _) = item_info
                .get(&answer.checklist_template_item_id)
                .cloned()
                .unwrap_or_default();
            let item = WorkOrderItem {
                id: None,
                uuid: None,
                tenant_id: run.tenant_id,
                work_order_id,
                description,
                item_type: None,
                status: WorkOrderItemStatus::Pending,
                observation: answer.observation.clone(),
                resolved_by: None,
                resolved_at: None,
                resolution_description: None,
                purchase_order_id: None,
                is_purchase_placeholder: false,
                preventive_plan_id: None,
                created_at: None,
                created_by: None,
                updated_at: None,
                updated_by: None,
            };
            self.store.save_work_order_item(item).await?;
        }

        Ok(work_order_id)
    }

    async fn validated(&self, run: ChecklistRun) -> Result<ChecklistRun, BusinessError> {
        let template = self.store.find_template(run.checklist_template_id).await?;
        if !template.is_some_and(|t| t.tenant_id == run.tenant_id) {
            return Err(BusinessError::new(NOT_A_TENANT_TEMPLATE.to_string()));
        }

        let vehicle = self.store.find_vehicle(run.vehicle_id).await?;
        let Some(vehicle) = vehicle.filter(|v| v.tenant_id == run.tenant_id) else {
            return Err(BusinessError::new(NOT_A_TENANT_VEHICLE.to_string()));
        };
        if run.odometer_km < 0.0 {
            return Err(BusinessError::new(ODOMETER_NEGATIVE.to_string()));
        }
        if vehicle.odometer_km.is_some_and(|official| run.odometer_km < official) {
            return Err(BusinessError::new(ODOMETER_BELOW_OFFICIAL.to_string()));
        }

        let driver = self.store.find_driver(run.driver_id).await?;
        let is_valid_driver = driver.is_some_and(|user| {
            user.role == Role::Driver && user.enabled && user.tenant_id == run.tenant_id
        });
        if !is_valid_driver {
            return Err(BusinessError::new(NOT_A_DRIVER.to_string()));
        }

        match run.checklist_type {
            ChecklistType::Departure | ChecklistType::Standalone => {
                if run.opening_checklist_id.is_some() {
                    return Err(BusinessError::new(
                        MAY_NOT_NAME_AN_OPENING_CHECKLIST.to_string(),
                    ));
                }
                if matches!(run.checklist_type, ChecklistType::Departure)
                    && self.driver_holds_an_open_vehicle(run.driver_id).await?
                {
                    return Err(BusinessError::new(
                        DRIVER_ALREADY_HOLDS_A_VEHICLE.to_string(),
                    ));
                }
            }
            ChecklistType::Return => {
                let Some(opening_id) = run.opening_checklist_id else {
                    return Err(BusinessError::new(
                        RETURN_REQUIRES_OPENING_CHECKLIST.to_string(),
                    ));
                };
                let opening = self.store.find_run(opening_id).await?;
                let Some(opening) = opening else {
                    return Err(BusinessError::new(OPENING_CHECKLIST_NOT_FOUND.to_string()));
                };
                if opening.vehicle_id != run.vehicle_id {
                    return Err(BusinessError::new(
                        OPENING_CHECKLIST_WRONG_VEHICLE.to_string(),
                    ));
                }
                let already_closed = !self
                    .store
                    .returns_closing(vec![opening_id])
                    .await?
                    .is_empty();
                if already_closed {
                    return Err(BusinessError::new(
                        OPENING_CHECKLIST_ALREADY_CLOSED.to_string(),
                    ));
                }
            }
        }

        Ok(run)
    }

    /// `TRM-105`: true when this driver has a `Departure` with no `Return`
    /// naming it yet.
    async fn driver_holds_an_open_vehicle(&self, driver_id: i64) -> Result<bool, BusinessError> {
        let departures = self.store.departures_by_driver(driver_id).await?;
        if departures.is_empty() {
            return Ok(false);
        }
        let ids: Vec<i64> = departures
            .iter()
            .filter_map(|departure| departure.id)
            .collect();
        let closed_ids: Vec<i64> = self
            .store
            .returns_closing(ids)
            .await?
            .into_iter()
            .filter_map(|r| r.opening_checklist_id)
            .collect();
        Ok(departures
            .iter()
            .any(|departure| departure.id.is_some_and(|id| !closed_ids.contains(&id))))
    }

    /// `TRM-123`-lite: the vehicle's current holder, or `None` when its
    /// latest `Departure` has already been closed by a `Return`.
    pub async fn current_holder(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<ChecklistRun>, BusinessError> {
        let Some(latest) = self.store.latest_departure_by_vehicle(vehicle_id).await? else {
            return Ok(None);
        };
        let closed = !self
            .store
            .returns_closing(vec![latest.id.unwrap_or_default()])
            .await?
            .is_empty();
        if closed { Ok(None) } else { Ok(Some(latest)) }
    }

    pub async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<(ChecklistRun, Vec<ChecklistAnswer>), BusinessError> {
        let run = match self.store.find_run_by_uuid(uuid).await? {
            Some(run) => run,
            None => return Err(BusinessError::new("Checklist not found".to_string())),
        };
        let answers = self.store.list_answers(run.id.unwrap_or_default()).await?;
        Ok((run, answers))
    }
}
