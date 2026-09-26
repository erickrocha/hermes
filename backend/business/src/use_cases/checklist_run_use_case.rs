use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::checklist_answer::{ChecklistAnswer, ChecklistAnswerEntityMapper};
use crate::domain::checklist_run::{ChecklistRun, ChecklistRunEntityMapper};
use crate::domain::enums::{
    AnswerStatus, ChecklistType, KmOrigin, Role, WorkOrderItemStatus, WorkOrderOrigin, WorkOrderStatus,
};
use crate::domain::km_evolution::KmEvolution;
use crate::domain::work_order::{WorkOrder, WorkOrderEntityMapper};
use crate::domain::work_order_item::WorkOrderItem;
use crate::gateway::checklist_answer_gateway::ChecklistAnswerGateway;
use crate::gateway::checklist_run_gateway::ChecklistRunGateway;
use crate::gateway::checklist_template_gateway::ChecklistTemplateGateway;
use crate::gateway::checklist_template_item_gateway::ChecklistTemplateItemGateway;
use crate::gateway::user_gateway::UserGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use crate::gateway::work_order_gateway::WorkOrderGateway;
use crate::gateway::work_order_item_gateway::WorkOrderItemGateway;
use crate::use_cases::km_evolution_use_case::KmEvolutionUseCase;
use sea_orm::{ActiveModelTrait, DbErr, TransactionTrait};
use std::collections::{HashMap, HashSet};

pub const NOT_A_TENANT_TEMPLATE: &str = "The checklist template named does not belong to this tenant";
pub const NOT_A_TENANT_VEHICLE: &str = "The vehicle named does not belong to this tenant";
pub const NOT_A_DRIVER: &str = "The person named is not an active driver of this tenant";
/// `TRM-105`: a driver may hold at most one open vehicle at a time.
pub const DRIVER_ALREADY_HOLDS_A_VEHICLE: &str =
    "This driver already holds an open checklist on another vehicle";
pub const RETURN_REQUIRES_OPENING_CHECKLIST: &str = "A return checklist must name the departure it closes";
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

pub struct ChecklistRunUseCase {
    gateway: ChecklistRunGateway,
    templates: ChecklistTemplateGateway,
    template_items: ChecklistTemplateItemGateway,
    answers: ChecklistAnswerGateway,
    vehicles: VehicleGateway,
    drivers: UserGateway,
    km_evolution: KmEvolutionUseCase,
    work_orders: WorkOrderGateway,
    work_order_items: WorkOrderItemGateway,
}

impl ChecklistRunUseCase {
    /// Nine collaborators is this use case coordinating six entities
    /// (`checklist_run`, `checklist_answer`, `checklist_template(_item)`,
    /// `vehicle`, `user`, and now `work_order(_item)` for `EPIC-CK-03-S02`),
    /// not a design smell to refactor away with a builder or config struct
    /// for two call sites (the endpoint and the test helper).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        gateway: ChecklistRunGateway,
        templates: ChecklistTemplateGateway,
        template_items: ChecklistTemplateItemGateway,
        answers: ChecklistAnswerGateway,
        vehicles: VehicleGateway,
        drivers: UserGateway,
        km_evolution: KmEvolutionUseCase,
        work_orders: WorkOrderGateway,
        work_order_items: WorkOrderItemGateway,
    ) -> Self {
        Self {
            gateway,
            templates,
            template_items,
            answers,
            vehicles,
            drivers,
            km_evolution,
            work_orders,
            work_order_items,
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

        let transaction = self.gateway.db().begin().await.map_err(database_error)?;

        let saved_run = match ChecklistRunEntityMapper::build_active_model(run)
            .save(&transaction)
            .await
        {
            Ok(active) => ChecklistRunEntityMapper::from_active_model(active),
            Err(e) => {
                let _ = transaction.rollback().await;
                return Err(database_error(e));
            }
        };

        let mut saved_answers = Vec::with_capacity(answers.len());
        for mut answer in answers {
            answer.checklist_run_id = saved_run.id.unwrap_or_default();
            match ChecklistAnswerEntityMapper::build_active_model(answer)
                .save(&transaction)
                .await
            {
                Ok(active) => {
                    saved_answers.push(ChecklistAnswerEntityMapper::from_active_model(active))
                }
                Err(e) => {
                    let _ = transaction.rollback().await;
                    return Err(database_error(e));
                }
            }
        }

        transaction.commit().await.map_err(database_error)?;

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
            let flagged_item_ids: HashSet<i64> =
                flagged.iter().map(|a| a.checklist_template_item_id).collect();
            for answer in saved_answers.iter_mut() {
                if flagged_item_ids.contains(&answer.checklist_template_item_id) {
                    answer.work_order_id = Some(work_order_id);
                    let entity = self
                        .answers
                        .persist(answer.clone())
                        .await
                        .map_err(database_error)?;
                    *answer = ChecklistAnswerEntityMapper::from_active_model(entity);
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
                .template_items
                .find_by_id(answer.checklist_template_item_id)
                .await
                .map_err(database_error)?;
            let belongs = item.as_ref().is_some_and(|i| {
                i.checklist_template_id == run.checklist_template_id && i.tenant_id == run.tenant_id
            });
            if !belongs {
                return Err(BusinessError::new(ANSWER_NOT_A_TEMPLATE_ITEM.to_string()));
            }
            let item = item.unwrap();
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
        let entity = self.work_orders.persist(work_order).await.map_err(database_error)?;
        let work_order_id = WorkOrderEntityMapper::from_active_model(entity)
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
                created_at: None,
                created_by: None,
                updated_at: None,
                updated_by: None,
            };
            self.work_order_items.persist(item).await.map_err(database_error)?;
        }

        Ok(work_order_id)
    }

    async fn validated(&self, run: ChecklistRun) -> Result<ChecklistRun, BusinessError> {
        let template = self
            .templates
            .find_by_id(run.checklist_template_id)
            .await
            .map_err(database_error)?;
        if !template.is_some_and(|t| t.tenant_id == run.tenant_id) {
            return Err(BusinessError::new(NOT_A_TENANT_TEMPLATE.to_string()));
        }

        let vehicle = self
            .vehicles
            .find_by_id(run.vehicle_id)
            .await
            .map_err(database_error)?;
        if !vehicle.is_some_and(|v| v.tenant_id == run.tenant_id) {
            return Err(BusinessError::new(NOT_A_TENANT_VEHICLE.to_string()));
        }

        let driver = self
            .drivers
            .find_by_id(run.driver_id)
            .await
            .map_err(database_error)?;
        let is_valid_driver = driver.is_some_and(|user| {
            user.role == Role::Driver.to_string() && user.enabled && user.tenant_id == run.tenant_id
        });
        if !is_valid_driver {
            return Err(BusinessError::new(NOT_A_DRIVER.to_string()));
        }

        match run.checklist_type {
            ChecklistType::Departure | ChecklistType::Standalone => {
                if run.opening_checklist_id.is_some() {
                    return Err(BusinessError::new(MAY_NOT_NAME_AN_OPENING_CHECKLIST.to_string()));
                }
                if matches!(run.checklist_type, ChecklistType::Departure)
                    && self.driver_holds_an_open_vehicle(run.driver_id).await?
                {
                    return Err(BusinessError::new(DRIVER_ALREADY_HOLDS_A_VEHICLE.to_string()));
                }
            }
            ChecklistType::Return => {
                let Some(opening_id) = run.opening_checklist_id else {
                    return Err(BusinessError::new(RETURN_REQUIRES_OPENING_CHECKLIST.to_string()));
                };
                let opening = self.gateway.find_by_id(opening_id).await.map_err(database_error)?;
                let Some(opening) = opening else {
                    return Err(BusinessError::new(OPENING_CHECKLIST_NOT_FOUND.to_string()));
                };
                if opening.vehicle_id != run.vehicle_id {
                    return Err(BusinessError::new(OPENING_CHECKLIST_WRONG_VEHICLE.to_string()));
                }
                let already_closed = !self
                    .gateway
                    .find_returns_closing(vec![opening_id])
                    .await
                    .map_err(database_error)?
                    .is_empty();
                if already_closed {
                    return Err(BusinessError::new(OPENING_CHECKLIST_ALREADY_CLOSED.to_string()));
                }
            }
        }

        Ok(run)
    }

    /// `TRM-105`: true when this driver has a `Departure` with no `Return`
    /// naming it yet.
    async fn driver_holds_an_open_vehicle(&self, driver_id: i64) -> Result<bool, BusinessError> {
        let departures = self
            .gateway
            .find_departures_by_driver(driver_id)
            .await
            .map_err(database_error)?;
        if departures.is_empty() {
            return Ok(false);
        }
        let ids: Vec<i64> = departures.iter().map(|d| d.id).collect();
        let closed_ids: Vec<i64> = self
            .gateway
            .find_returns_closing(ids)
            .await
            .map_err(database_error)?
            .into_iter()
            .filter_map(|r| r.opening_checklist_id)
            .collect();
        Ok(departures.iter().any(|d| !closed_ids.contains(&d.id)))
    }

    /// `TRM-123`-lite: the vehicle's current holder, or `None` when its
    /// latest `Departure` has already been closed by a `Return`.
    pub async fn current_holder(&self, vehicle_id: i64) -> Result<Option<ChecklistRun>, BusinessError> {
        let Some(latest) = self
            .gateway
            .find_latest_departure_by_vehicle(vehicle_id)
            .await
            .map_err(database_error)?
        else {
            return Ok(None);
        };
        let closed = !self
            .gateway
            .find_returns_closing(vec![latest.id])
            .await
            .map_err(database_error)?
            .is_empty();
        if closed {
            Ok(None)
        } else {
            Ok(Some(ChecklistRunEntityMapper::from_model(latest)))
        }
    }

    pub async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<(ChecklistRun, Vec<ChecklistAnswer>), BusinessError> {
        let entity = self.gateway.find_by_uuid(uuid).await.map_err(database_error)?;
        let run = match entity {
            Some(model) => ChecklistRunEntityMapper::from_model(model),
            None => return Err(BusinessError::new("Checklist not found".to_string())),
        };
        let answers = self
            .answers
            .find_by_run(run.id.unwrap_or_default())
            .await
            .map_err(database_error)?;
        Ok((run, ChecklistAnswerEntityMapper::from_models(answers)))
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[ChecklistRunUseCase] {}", msg);
    BusinessError::new(msg)
}
