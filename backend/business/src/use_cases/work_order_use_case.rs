use crate::domain::business_error::BusinessError;
use crate::domain::enums::{KmOrigin, WorkOrderItemStatus, WorkOrderOrigin, WorkOrderStatus};
use crate::domain::km_evolution::KmEvolution;
use crate::domain::preventive_plan::PreventivePlan;
use crate::domain::work_order::WorkOrder;
use crate::domain::work_order_item::WorkOrderItem;
use crate::gateway::work_order_gateway::WorkOrderWorkflowStore;
use crate::use_cases::km_evolution_use_case::KmEvolutionUseCase;
use chrono::Utc;

pub const NOT_A_TENANT_VEHICLE: &str = "The vehicle named does not belong to this tenant";
pub const DESCRIPTION_REQUIRED: &str = "A work order needs a description";
pub const NEGATIVE_ODOMETER: &str = "Odometer reading cannot be negative";
pub const ITEM_DESCRIPTION_REQUIRED: &str = "A work order item needs a description";
pub const WORK_ORDER_NOT_FOUND: &str = "Work order not found";
pub const PLAN_NOT_ON_THIS_VEHICLE: &str =
    "The preventive plan does not belong to this work order's vehicle";
pub const ITEM_NOT_FOUND: &str = "Work order item not found";
pub const ITEM_NOT_ON_THIS_WORK_ORDER: &str = "This item does not belong to the named work order";
pub const RESOLUTION_REQUIRED: &str = "A resolution description is required";
pub const WORK_ORDER_CLOSED: &str = "The work order is already concluded or cancelled";

pub struct WorkOrderUseCase {
    store: Box<dyn WorkOrderWorkflowStore>,
    km_evolution: KmEvolutionUseCase,
}

impl WorkOrderUseCase {
    pub fn new<S: WorkOrderWorkflowStore + 'static>(
        store: S,
        km_evolution: KmEvolutionUseCase,
    ) -> Self {
        Self {
            store: Box::new(store),
            km_evolution,
        }
    }

    /// `HRMS-700`/`TRM-202`: opening a work order and recording its
    /// odometer reading are one operation -- the reading goes through the
    /// one official writer `EPIC-CK-01-S01` built (`AD-041`), the same
    /// wiring `ChecklistRunUseCase::create` already established for
    /// `KmOrigin::DriverChecklist`, here for `KmOrigin::WorkOrder`.
    pub async fn create(&self, work_order: WorkOrder) -> Result<WorkOrder, BusinessError> {
        let work_order = self.validated(work_order).await?;
        let saved = self.store.save_work_order(work_order).await?;

        let reading = KmEvolution {
            id: None,
            uuid: None,
            tenant_id: saved.tenant_id,
            vehicle_id: saved.vehicle_id,
            km: saved.odometer_km,
            recorded_at: saved.opened_at,
            origin: KmOrigin::WorkOrder,
            source_entity: Some("work_order".to_string()),
            source_entity_id: saved.id,
            notes: None,
            recorded_by_user_id: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        };
        self.km_evolution.create(reading).await?;

        Ok(saved)
    }

    async fn validated(&self, work_order: WorkOrder) -> Result<WorkOrder, BusinessError> {
        let description = work_order.description.trim().to_string();
        if description.is_empty() {
            return Err(BusinessError::new(DESCRIPTION_REQUIRED.to_string()));
        }
        if work_order.odometer_km < 0.0 {
            return Err(BusinessError::new(NEGATIVE_ODOMETER.to_string()));
        }

        let vehicle = self.store.find_vehicle(work_order.vehicle_id).await?;
        if !vehicle.is_some_and(|v| v.tenant_id == work_order.tenant_id) {
            return Err(BusinessError::new(NOT_A_TENANT_VEHICLE.to_string()));
        }

        Ok(WorkOrder {
            description,
            origin: WorkOrderOrigin::Manual,
            status: WorkOrderStatus::Open,
            // `HRMS-700`: the opening time is the system's. A client-sent time is not trusted: it
            // would also backdate the kilometre entry this order records and so change which
            // reading is "latest".
            opened_at: None,
            ..work_order
        })
    }

    pub async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<(WorkOrder, Vec<WorkOrderItem>), BusinessError> {
        let work_order = match self.store.find_work_order_by_uuid(uuid).await? {
            Some(work_order) => work_order,
            None => return Err(BusinessError::new(WORK_ORDER_NOT_FOUND.to_string())),
        };
        let items = self
            .store
            .list_work_order_items(work_order.id.unwrap_or_default())
            .await?;
        Ok((work_order, items))
    }

    /// `HRMS-701`/`TRM-203`: adding an item and recomputing the parent
    /// work order's status are one operation -- `TRM-206`/`TRM-207` decide
    /// the new status from the full item set, and `TRM-215` limits when a
    /// terminal (`Concluded`/`Cancelled`) work order may be touched at all.
    pub async fn add_item(
        &self,
        work_order_id: i64,
        item: WorkOrderItem,
    ) -> Result<(WorkOrder, WorkOrderItem), BusinessError> {
        let description = item.description.trim().to_string();
        if description.is_empty() {
            return Err(BusinessError::new(ITEM_DESCRIPTION_REQUIRED.to_string()));
        }

        let existing = self.store.find_work_order(work_order_id).await?;
        let Some(existing) = existing else {
            return Err(BusinessError::new(WORK_ORDER_NOT_FOUND.to_string()));
        };
        let work_order = existing;

        if let Some(plan_id) = item.preventive_plan_id {
            let plan = self.store.find_preventive_plan(plan_id).await?;
            if !plan.is_some_and(|p| p.vehicle_id == work_order.vehicle_id) {
                return Err(BusinessError::new(PLAN_NOT_ON_THIS_VEHICLE.to_string()));
            }
        }

        let item = WorkOrderItem {
            work_order_id,
            tenant_id: work_order.tenant_id,
            description,
            ..item
        };
        let saved_item = self.store.save_work_order_item(item).await?;

        let all_items = self.store.list_work_order_items(work_order_id).await?;

        let new_status = Self::recompute_status(&work_order.status, &saved_item, &all_items);
        let updated_work_order = if new_status == work_order.status {
            work_order
        } else {
            self.store
                .save_work_order(WorkOrder {
                    status: new_status,
                    ..work_order
                })
                .await?
        };

        Ok((updated_work_order, saved_item))
    }

    /// `TRM-215`: a terminal work order is left untouched unless the item
    /// just added is a genuinely new `Pending` one. `TRM-206`: any
    /// `AwaitingParts` item holds the order in `AwaitingParts`. `TRM-207`:
    /// `PartiallyResolved` means work has been done (something already
    /// `Resolved`/`Cancelled`) and a pendency remains. Conclusion
    /// (`TRM-208`) is a deliberate administrative act, never derived here --
    /// an item set with nothing left pending leaves the status untouched.
    pub(crate) fn recompute_status(
        current: &WorkOrderStatus,
        new_item: &WorkOrderItem,
        items: &[WorkOrderItem],
    ) -> WorkOrderStatus {
        let terminal = matches!(
            current,
            WorkOrderStatus::Concluded | WorkOrderStatus::Cancelled
        );
        if terminal && new_item.status != WorkOrderItemStatus::Pending {
            return current.clone();
        }

        let any_awaiting_parts = items
            .iter()
            .any(|i| i.status == WorkOrderItemStatus::AwaitingParts);
        let any_pending = items
            .iter()
            .any(|i| i.status == WorkOrderItemStatus::Pending);
        let any_settled = items.iter().any(|i| {
            matches!(
                i.status,
                WorkOrderItemStatus::Resolved | WorkOrderItemStatus::Cancelled
            )
        });

        if any_awaiting_parts {
            WorkOrderStatus::AwaitingParts
        } else if any_pending && any_settled {
            WorkOrderStatus::PartiallyResolved
        } else if any_pending {
            WorkOrderStatus::Open
        } else if *current == WorkOrderStatus::AwaitingParts {
            // `TRM-649`: nothing awaits a part any more, so the order leaves "awaiting parts" -- the
            // received part is no longer what holds the repair. (Concluding stays an administrative
            // act, `TRM-208`; `Open` here is the in-execution state.) Without this, an order whose only
            // pendency was the purchase placeholder stayed `AwaitingParts` for ever after receipt.
            WorkOrderStatus::Open
        } else {
            current.clone()
        }
    }

    /// `EPIC-SP-03-S02` (`TRM-644`/`646`): names an existing pendency as the
    /// one a purchase order was raised from -- the item moves to
    /// `AwaitingParts` and is annotated with the order, and the work order's
    /// own status is recomputed the same way `add_item` recomputes it.
    pub async fn mark_item_awaiting_parts(
        &self,
        work_order_id: i64,
        item_id: i64,
        purchase_order_id: i64,
    ) -> Result<WorkOrder, BusinessError> {
        let work_order = self.loaded(work_order_id).await?;
        let item = self.loaded_item(item_id).await?;
        if item.work_order_id != work_order_id {
            return Err(BusinessError::new(ITEM_NOT_ON_THIS_WORK_ORDER.to_string()));
        }

        let updated_item = WorkOrderItem {
            status: WorkOrderItemStatus::AwaitingParts,
            purchase_order_id: Some(purchase_order_id),
            is_purchase_placeholder: false,
            preventive_plan_id: None,
            ..item
        };
        self.recomputed_after(work_order, updated_item).await
    }

    /// `EPIC-SP-03-S02` (`TRM-647`/`648`/`649`): on receipt, a synthetic
    /// placeholder pendency (`TRM-645`) is resolved outright -- it only ever
    /// represented the purchase -- while a real pendency (`TRM-644`) returns
    /// to `Pending` with its purchase-order annotation cleared, because
    /// receiving the part does not mean the repair was done. Either way the
    /// work order's own status is recomputed, which is `TRM-649`'s "move
    /// from awaiting-part to in-execution" the moment no item still awaits
    /// one.
    pub async fn resolve_purchase_pendency(
        &self,
        item_id: i64,
    ) -> Result<WorkOrder, BusinessError> {
        let item = self.loaded_item(item_id).await?;
        let work_order = self.loaded(item.work_order_id).await?;

        let updated_item = if item.is_purchase_placeholder {
            WorkOrderItem {
                status: WorkOrderItemStatus::Resolved,
                resolved_at: Some(Utc::now().naive_utc()),
                ..item
            }
        } else {
            WorkOrderItem {
                status: WorkOrderItemStatus::Pending,
                purchase_order_id: None,
                ..item
            }
        };
        self.recomputed_after(work_order, updated_item).await
    }

    /// `TRM-205`: the mechanic closes out one pendency of an open order, saying what was done. The order's
    /// own status is recomputed like every other item change (`TRM-206`/`207`); concluding the order stays the
    /// administrative act it is (`TRM-208`). A concluded or cancelled order takes no more work.
    pub async fn resolve_item(
        &self,
        work_order_id: i64,
        item_id: i64,
        resolution: &str,
        resolved_by: &str,
    ) -> Result<WorkOrder, BusinessError> {
        let resolution = resolution.trim();
        if resolution.is_empty() {
            return Err(BusinessError::new(RESOLUTION_REQUIRED.to_string()));
        }
        let work_order = self.loaded(work_order_id).await?;
        if matches!(work_order.status, WorkOrderStatus::Concluded | WorkOrderStatus::Cancelled) {
            return Err(BusinessError::new(WORK_ORDER_CLOSED.to_string()));
        }
        let item = self.loaded_item(item_id).await?;
        if item.work_order_id != work_order_id {
            return Err(BusinessError::new(ITEM_NOT_ON_THIS_WORK_ORDER.to_string()));
        }
        let resolved = WorkOrderItem {
            status: WorkOrderItemStatus::Resolved,
            resolved_by: Some(resolved_by.to_string()),
            resolved_at: Some(Utc::now().naive_utc()),
            resolution_description: Some(resolution.to_string()),
            ..item
        };
        self.recomputed_after(work_order, resolved).await
    }

    pub(crate) async fn loaded(&self, work_order_id: i64) -> Result<WorkOrder, BusinessError> {
        let existing = self.store.find_work_order(work_order_id).await?;
        match existing {
            Some(work_order) => Ok(work_order),
            None => Err(BusinessError::new(WORK_ORDER_NOT_FOUND.to_string())),
        }
    }

    pub(crate) async fn loaded_item(&self, item_id: i64) -> Result<WorkOrderItem, BusinessError> {
        let existing = self.store.find_work_order_item(item_id).await?;
        match existing {
            Some(item) => Ok(item),
            None => Err(BusinessError::new(ITEM_NOT_FOUND.to_string())),
        }
    }

    /// Shared by `mark_item_awaiting_parts` and `resolve_purchase_pendency`:
    /// persist the item's new state, then recompute and persist the parent
    /// work order's status from the full item set -- the same two-step
    /// `add_item` already uses.
    async fn recomputed_after(
        &self,
        work_order: WorkOrder,
        item: WorkOrderItem,
    ) -> Result<WorkOrder, BusinessError> {
        let saved_item = self.store.save_work_order_item(item).await?;
        let all_items = self
            .store
            .list_work_order_items(work_order.id.unwrap_or_default())
            .await?;

        let new_status = Self::recompute_status(&work_order.status, &saved_item, &all_items);
        if new_status == work_order.status {
            Ok(work_order)
        } else {
            self.store
                .save_work_order(WorkOrder {
                    status: new_status,
                    ..work_order
                })
                .await
        }
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<WorkOrder>, u64), BusinessError> {
        self.store.list_work_orders(page, page_size).await
    }

    /// `TRM-208`/`TRM-214`: conclusion is a deliberate administrative act,
    /// never derived from item state alone. `TRM-204`: attempting to
    /// conclude a work order that still has a pending or awaiting-parts
    /// item does not fail and does not conclude it either -- it downgrades
    /// to `PartiallyResolved`, leaving every item untouched. `TRM-215`: a
    /// work order already `Concluded`/`Cancelled` is left exactly as it is.
    pub async fn conclude(&self, work_order_id: i64) -> Result<WorkOrder, BusinessError> {
        let Some(work_order) = self.store.find_work_order(work_order_id).await? else {
            return Err(BusinessError::new(WORK_ORDER_NOT_FOUND.to_string()));
        };
        if Self::is_terminal(&work_order.status) {
            return Ok(work_order);
        }

        let items = self.store.list_work_order_items(work_order_id).await?;
        let all_settled = items.iter().all(|i| {
            matches!(
                i.status,
                WorkOrderItemStatus::Resolved | WorkOrderItemStatus::Cancelled
            )
        });

        let (status, concluded_at) = if all_settled {
            (WorkOrderStatus::Concluded, Some(Utc::now().date_naive()))
        } else {
            (WorkOrderStatus::PartiallyResolved, work_order.concluded_at)
        };

        let concluded = self
            .store
            .save_work_order(WorkOrder {
                status,
                concluded_at,
                ..work_order
            })
            .await?;
        if concluded.status == WorkOrderStatus::Concluded {
            self.renew_preventive_cycles(&concluded, &items).await?;
        }
        Ok(concluded)
    }

    /// `TRM-309`/`TRM-310`/`TRM-329`: concluding a work order opens the next
    /// cycle of every plan it serviced -- the order's own plan plus the plan
    /// each *resolved* item names, so one order can close several plans
    /// without mixing their cycles. The conclusion date and odometer (the
    /// vehicle's current one, the order's frozen one if none) become the last
    /// service, and every other order still open for that plan is cancelled.
    /// Runs only on the real transition to `Concluded` (an already-terminal
    /// order returns before reaching here), so it is never applied twice.
    async fn renew_preventive_cycles(
        &self,
        work_order: &WorkOrder,
        items: &[WorkOrderItem],
    ) -> Result<(), BusinessError> {
        // `TRM-311`: an item resolved by an extension is not a service, so it
        // renews nothing -- and neither does the order's own plan link when an
        // extension item names that plan.
        let mut plan_ids: Vec<i64> = Vec::new();
        let mut extended_plans: Vec<i64> = Vec::new();
        for item in items
            .iter()
            .filter(|i| i.status == WorkOrderItemStatus::Resolved)
        {
            let Some(plan_id) = item.preventive_plan_id else {
                continue;
            };
            let extended = self
                .store
                .find_extension_by_item(item.id.unwrap_or_default())
                .await?
                .is_some();
            if extended {
                extended_plans.push(plan_id);
            } else if !plan_ids.contains(&plan_id) {
                plan_ids.push(plan_id);
            }
        }
        if let Some(id) = work_order.preventive_plan_id
            && !plan_ids.contains(&id)
            && !extended_plans.contains(&id)
        {
            plan_ids.push(id);
        }
        if plan_ids.is_empty() {
            return Ok(());
        }

        let current_km = self
            .store
            .find_vehicle(work_order.vehicle_id)
            .await?
            .and_then(|vehicle| vehicle.odometer_km);

        for plan_id in plan_ids {
            let Some(plan) = self.store.find_preventive_plan(plan_id).await? else {
                continue;
            };
            let plan = PreventivePlan {
                last_service_km: Some(current_km.unwrap_or(work_order.odometer_km)),
                last_service_date: work_order.concluded_at,
                // `TRM-309`: a real service ends any active extension; the
                // extension entries themselves are kept.
                extension_limit_km: None,
                last_work_order_id: work_order.id,
                ..plan
            };
            self.store.save_preventive_plan(plan).await?;

            let others = self.store.find_active_work_orders_by_plan(plan_id).await?;
            for other in others {
                if let Some(id) = other.id {
                    self.cancel(id).await?;
                }
            }
        }
        Ok(())
    }

    /// `TRM-205`: cancelling closes a work order regardless of outstanding
    /// items, because cancelling is the act of abandoning what is left.
    /// `TRM-215`: already-terminal is left untouched.
    pub async fn cancel(&self, work_order_id: i64) -> Result<WorkOrder, BusinessError> {
        let Some(work_order) = self.store.find_work_order(work_order_id).await? else {
            return Err(BusinessError::new(WORK_ORDER_NOT_FOUND.to_string()));
        };
        if Self::is_terminal(&work_order.status) {
            return Ok(work_order);
        }

        self.store
            .save_work_order(WorkOrder {
                status: WorkOrderStatus::Cancelled,
                concluded_at: Some(Utc::now().date_naive()),
                ..work_order
            })
            .await
    }

    fn is_terminal(status: &WorkOrderStatus) -> bool {
        matches!(
            status,
            WorkOrderStatus::Concluded | WorkOrderStatus::Cancelled
        )
    }
}

#[cfg(test)]
mod recompute_status_tests {
    use super::*;

    fn item(status: WorkOrderItemStatus) -> WorkOrderItem {
        WorkOrderItem {
            id: None,
            uuid: None,
            tenant_id: None,
            work_order_id: 1,
            description: "x".into(),
            item_type: None,
            status,
            observation: None,
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
        }
    }

    /// `TRM-649`: when the last item awaiting a part is settled, the order leaves `AwaitingParts`.
    #[test]
    fn an_order_leaves_awaiting_parts_when_nothing_awaits_a_part_any_more() {
        let resolved = item(WorkOrderItemStatus::Resolved);
        let only_settled = vec![resolved.clone()];
        assert_eq!(
            WorkOrderUseCase::recompute_status(
                &WorkOrderStatus::AwaitingParts,
                &resolved,
                &only_settled
            ),
            WorkOrderStatus::Open
        );
        // still held while another item awaits
        let held = vec![resolved.clone(), item(WorkOrderItemStatus::AwaitingParts)];
        assert_eq!(
            WorkOrderUseCase::recompute_status(&WorkOrderStatus::AwaitingParts, &resolved, &held),
            WorkOrderStatus::AwaitingParts
        );
        // conclusion is never derived: an Open order with only settled items stays Open
        assert_eq!(
            WorkOrderUseCase::recompute_status(&WorkOrderStatus::Open, &resolved, &only_settled),
            WorkOrderStatus::Open
        );
    }
}
