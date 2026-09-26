use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::{KmOrigin, WorkOrderItemStatus, WorkOrderOrigin, WorkOrderStatus};
use crate::domain::km_evolution::KmEvolution;
use crate::domain::work_order::{WorkOrder, WorkOrderEntityMapper};
use crate::domain::work_order_item::{WorkOrderItem, WorkOrderItemEntityMapper};
use crate::gateway::vehicle_gateway::VehicleGateway;
use crate::gateway::work_order_gateway::WorkOrderGateway;
use crate::gateway::work_order_item_gateway::WorkOrderItemGateway;
use crate::use_cases::km_evolution_use_case::KmEvolutionUseCase;
use chrono::Utc;
use sea_orm::DbErr;

pub const NOT_A_TENANT_VEHICLE: &str = "The vehicle named does not belong to this tenant";
pub const DESCRIPTION_REQUIRED: &str = "A work order needs a description";
pub const NEGATIVE_ODOMETER: &str = "Odometer reading cannot be negative";
pub const ITEM_DESCRIPTION_REQUIRED: &str = "A work order item needs a description";
pub const WORK_ORDER_NOT_FOUND: &str = "Work order not found";

pub struct WorkOrderUseCase {
    gateway: WorkOrderGateway,
    items: WorkOrderItemGateway,
    vehicles: VehicleGateway,
    km_evolution: KmEvolutionUseCase,
}

impl WorkOrderUseCase {
    pub fn new(
        gateway: WorkOrderGateway,
        items: WorkOrderItemGateway,
        vehicles: VehicleGateway,
        km_evolution: KmEvolutionUseCase,
    ) -> Self {
        Self {
            gateway,
            items,
            vehicles,
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
        let entity = self.gateway.persist(work_order).await.map_err(database_error)?;
        let saved = WorkOrderEntityMapper::from_active_model(entity);

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

        let vehicle = self
            .vehicles
            .find_by_id(work_order.vehicle_id)
            .await
            .map_err(database_error)?;
        if !vehicle.is_some_and(|v| v.tenant_id == work_order.tenant_id) {
            return Err(BusinessError::new(NOT_A_TENANT_VEHICLE.to_string()));
        }

        Ok(WorkOrder {
            description,
            origin: WorkOrderOrigin::Manual,
            status: WorkOrderStatus::Open,
            ..work_order
        })
    }

    pub async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<(WorkOrder, Vec<WorkOrderItem>), BusinessError> {
        let entity = self.gateway.find_by_uuid(uuid).await.map_err(database_error)?;
        let work_order = match entity {
            Some(model) => WorkOrderEntityMapper::from_model(model),
            None => return Err(BusinessError::new(WORK_ORDER_NOT_FOUND.to_string())),
        };
        let items = self
            .items
            .find_by_work_order(work_order.id.unwrap_or_default())
            .await
            .map_err(database_error)?;
        Ok((work_order, WorkOrderItemEntityMapper::from_models(items)))
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

        let existing = self.gateway.find_by_id(work_order_id).await.map_err(database_error)?;
        let Some(existing) = existing else {
            return Err(BusinessError::new(WORK_ORDER_NOT_FOUND.to_string()));
        };
        let work_order = WorkOrderEntityMapper::from_model(existing);

        let item = WorkOrderItem {
            work_order_id,
            tenant_id: work_order.tenant_id,
            description,
            ..item
        };
        let entity = self.items.persist(item).await.map_err(database_error)?;
        let saved_item = WorkOrderItemEntityMapper::from_active_model(entity);

        let all_items = self
            .items
            .find_by_work_order(work_order_id)
            .await
            .map_err(database_error)?;
        let all_items = WorkOrderItemEntityMapper::from_models(all_items);

        let new_status = Self::recompute_status(&work_order.status, &saved_item, &all_items);
        let updated_work_order = if new_status == work_order.status {
            work_order
        } else {
            let entity = self
                .gateway
                .persist(WorkOrder {
                    status: new_status,
                    ..work_order
                })
                .await
                .map_err(database_error)?;
            WorkOrderEntityMapper::from_active_model(entity)
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
    fn recompute_status(
        current: &WorkOrderStatus,
        new_item: &WorkOrderItem,
        items: &[WorkOrderItem],
    ) -> WorkOrderStatus {
        let terminal = matches!(current, WorkOrderStatus::Concluded | WorkOrderStatus::Cancelled);
        if terminal && new_item.status != WorkOrderItemStatus::Pending {
            return current.clone();
        }

        let any_awaiting_parts = items.iter().any(|i| i.status == WorkOrderItemStatus::AwaitingParts);
        let any_pending = items.iter().any(|i| i.status == WorkOrderItemStatus::Pending);
        let any_settled = items
            .iter()
            .any(|i| matches!(i.status, WorkOrderItemStatus::Resolved | WorkOrderItemStatus::Cancelled));

        if any_awaiting_parts {
            WorkOrderStatus::AwaitingParts
        } else if any_pending && any_settled {
            WorkOrderStatus::PartiallyResolved
        } else if any_pending {
            WorkOrderStatus::Open
        } else {
            current.clone()
        }
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<WorkOrder>, u64), BusinessError> {
        let (rows, total) = self.gateway.find_page(page, page_size).await.map_err(database_error)?;
        Ok((WorkOrderEntityMapper::from_models(rows), total))
    }

    /// `TRM-208`/`TRM-214`: conclusion is a deliberate administrative act,
    /// never derived from item state alone. `TRM-204`: attempting to
    /// conclude a work order that still has a pending or awaiting-parts
    /// item does not fail and does not conclude it either -- it downgrades
    /// to `PartiallyResolved`, leaving every item untouched. `TRM-215`: a
    /// work order already `Concluded`/`Cancelled` is left exactly as it is.
    pub async fn conclude(&self, work_order_id: i64) -> Result<WorkOrder, BusinessError> {
        let existing = self.gateway.find_by_id(work_order_id).await.map_err(database_error)?;
        let Some(existing) = existing else {
            return Err(BusinessError::new(WORK_ORDER_NOT_FOUND.to_string()));
        };
        let work_order = WorkOrderEntityMapper::from_model(existing);
        if Self::is_terminal(&work_order.status) {
            return Ok(work_order);
        }

        let items = self
            .items
            .find_by_work_order(work_order_id)
            .await
            .map_err(database_error)?;
        let items = WorkOrderItemEntityMapper::from_models(items);
        let all_settled = items
            .iter()
            .all(|i| matches!(i.status, WorkOrderItemStatus::Resolved | WorkOrderItemStatus::Cancelled));

        let (status, concluded_at) = if all_settled {
            (WorkOrderStatus::Concluded, Some(Utc::now().date_naive()))
        } else {
            (WorkOrderStatus::PartiallyResolved, work_order.concluded_at)
        };

        let entity = self
            .gateway
            .persist(WorkOrder {
                status,
                concluded_at,
                ..work_order
            })
            .await
            .map_err(database_error)?;
        Ok(WorkOrderEntityMapper::from_active_model(entity))
    }

    /// `TRM-205`: cancelling closes a work order regardless of outstanding
    /// items, because cancelling is the act of abandoning what is left.
    /// `TRM-215`: already-terminal is left untouched.
    pub async fn cancel(&self, work_order_id: i64) -> Result<WorkOrder, BusinessError> {
        let existing = self.gateway.find_by_id(work_order_id).await.map_err(database_error)?;
        let Some(existing) = existing else {
            return Err(BusinessError::new(WORK_ORDER_NOT_FOUND.to_string()));
        };
        let work_order = WorkOrderEntityMapper::from_model(existing);
        if Self::is_terminal(&work_order.status) {
            return Ok(work_order);
        }

        let entity = self
            .gateway
            .persist(WorkOrder {
                status: WorkOrderStatus::Cancelled,
                concluded_at: Some(Utc::now().date_naive()),
                ..work_order
            })
            .await
            .map_err(database_error)?;
        Ok(WorkOrderEntityMapper::from_active_model(entity))
    }

    fn is_terminal(status: &WorkOrderStatus) -> bool {
        matches!(status, WorkOrderStatus::Concluded | WorkOrderStatus::Cancelled)
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[WorkOrderUseCase] {}", msg);
    BusinessError::new(msg)
}
