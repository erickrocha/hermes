use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::{PurchaseOrderStatus, WorkOrderItemStatus};
use crate::domain::purchase_order::{PurchaseOrder, PurchaseOrderEntityMapper};
use crate::domain::work_order_item::WorkOrderItem;
use crate::gateway::part_gateway::PartGateway;
use crate::gateway::purchase_order_gateway::PurchaseOrderGateway;
use crate::use_cases::work_order_use_case::WorkOrderUseCase;
use chrono::NaiveDate;
use sea_orm::DbErr;

pub const PART_NOT_FOUND: &str = "Part not found";
pub const PURCHASE_ORDER_NOT_FOUND: &str = "Purchase order not found";
/// `TRM-640`: a purchase order is always for a positive quantity.
pub const QUANTITY_MUST_BE_POSITIVE: &str = "A purchase order quantity must be positive";
/// `TRM-642`: "refuse to create a second active purchase order for the same
/// ... part on the same work order."
pub const DUPLICATE_ACTIVE_PURCHASE_ORDER: &str =
    "This work order already has an active purchase order for this part";
/// `TRM-641`: the state machine only moves Requested -> Ordered -> Purchased.
pub const WRONG_STATUS_FOR_TRANSITION: &str = "This purchase order is not in the right status for that transition";
/// `TRM-644`: a pendency is always a pendency *of* a work order.
pub const PENDENCY_REQUIRES_WORK_ORDER: &str = "A pendency may only be named together with its own work order";

pub struct PurchaseOrderUseCase {
    gateway: PurchaseOrderGateway,
    parts: PartGateway,
    work_orders: WorkOrderUseCase,
}

impl PurchaseOrderUseCase {
    pub fn new(gateway: PurchaseOrderGateway, parts: PartGateway, work_orders: WorkOrderUseCase) -> Self {
        Self {
            gateway,
            parts,
            work_orders,
        }
    }

    /// `TRM-640…646`: the purchase order itself, and -- when it is raised
    /// against a work order -- the pendency wiring `TRM-644`/`645` describe.
    /// Naming an existing `work_order_item_id` links that pendency
    /// (`TRM-644`); naming none creates a synthetic placeholder pendency
    /// that represents the purchase itself (`TRM-645`). Either way the work
    /// order's own status is recomputed to `AwaitingParts` (`TRM-646`) by
    /// the same `WorkOrderUseCase` machinery `add_item` already uses.
    pub async fn create(&self, purchase_order: PurchaseOrder) -> Result<PurchaseOrder, BusinessError> {
        if purchase_order.quantity <= 0.0 {
            return Err(BusinessError::new(QUANTITY_MUST_BE_POSITIVE.to_string()));
        }
        if purchase_order.work_order_item_id.is_some() && purchase_order.work_order_id.is_none() {
            return Err(BusinessError::new(PENDENCY_REQUIRES_WORK_ORDER.to_string()));
        }
        let part = self
            .parts
            .find_by_id(purchase_order.part_id)
            .await
            .map_err(database_error)?
            .ok_or_else(|| BusinessError::new(PART_NOT_FOUND.to_string()))?;
        if let Some(work_order_id) = purchase_order.work_order_id {
            let existing = self
                .gateway
                .find_active_by_part_and_work_order(purchase_order.part_id, work_order_id)
                .await
                .map_err(database_error)?;
            if !existing.is_empty() {
                return Err(BusinessError::new(DUPLICATE_ACTIVE_PURCHASE_ORDER.to_string()));
            }
        }

        let named_item_id = purchase_order.work_order_item_id;
        let entity = self
            .gateway
            .persist(PurchaseOrder {
                work_order_item_id: None,
                ..purchase_order
            })
            .await
            .map_err(database_error)?;
        let mut saved = PurchaseOrderEntityMapper::from_active_model(entity);

        if let Some(work_order_id) = saved.work_order_id {
            let item_id = match named_item_id {
                Some(item_id) => {
                    self.work_orders
                        .mark_item_awaiting_parts(work_order_id, item_id, saved.id.unwrap_or_default())
                        .await?;
                    item_id
                }
                None => {
                    let placeholder = WorkOrderItem {
                        id: None,
                        uuid: None,
                        tenant_id: saved.tenant_id,
                        work_order_id,
                        description: format!("Purchase order for {}", part.name),
                        item_type: None,
                        status: WorkOrderItemStatus::AwaitingParts,
                        observation: None,
                        resolved_by: None,
                        resolved_at: None,
                        resolution_description: None,
                        purchase_order_id: saved.id,
                        is_purchase_placeholder: true,
                        created_at: None,
                        created_by: None,
                        updated_at: None,
                        updated_by: None,
                    };
                    let (_, saved_item) = self.work_orders.add_item(work_order_id, placeholder).await?;
                    saved_item.id.unwrap_or_default()
                }
            };

            let entity = self
                .gateway
                .persist(PurchaseOrder {
                    work_order_item_id: Some(item_id),
                    ..saved
                })
                .await
                .map_err(database_error)?;
            saved = PurchaseOrderEntityMapper::from_active_model(entity);
        }

        Ok(saved)
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<PurchaseOrder, BusinessError> {
        let entity = self.gateway.find_by_uuid(uuid).await.map_err(database_error)?;
        match entity {
            Some(model) => Ok(PurchaseOrderEntityMapper::from_model(model)),
            None => Err(BusinessError::new(PURCHASE_ORDER_NOT_FOUND.to_string())),
        }
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<PurchaseOrder>, u64), BusinessError> {
        let (rows, total) = self.gateway.find_page(page, page_size).await.map_err(database_error)?;
        Ok((PurchaseOrderEntityMapper::from_models(rows), total))
    }

    /// `TRM-641`/`651`: `Requested` -> `Ordered`, capturing when it was
    /// placed and, optionally, when it is expected.
    pub async fn mark_ordered(
        &self,
        id: i64,
        ordered_at: Option<NaiveDate>,
        expected_delivery_date: Option<NaiveDate>,
    ) -> Result<PurchaseOrder, BusinessError> {
        let purchase_order = self.loaded(id).await?;
        if purchase_order.status != PurchaseOrderStatus::Requested {
            return Err(BusinessError::new(WRONG_STATUS_FOR_TRANSITION.to_string()));
        }
        let updated = PurchaseOrder {
            status: PurchaseOrderStatus::Ordered,
            ordered_at,
            expected_delivery_date,
            ..purchase_order
        };
        let entity = self.gateway.persist(updated).await.map_err(database_error)?;
        Ok(PurchaseOrderEntityMapper::from_active_model(entity))
    }

    /// `TRM-641`: `Ordered` -> `Purchased`. `TRM-647`/`648`/`649`: on
    /// receipt, the linked pendency (if any) resolves -- see
    /// `WorkOrderUseCase::resolve_purchase_pendency`.
    pub async fn mark_purchased(&self, id: i64) -> Result<PurchaseOrder, BusinessError> {
        let purchase_order = self.loaded(id).await?;
        if purchase_order.status != PurchaseOrderStatus::Ordered {
            return Err(BusinessError::new(WRONG_STATUS_FOR_TRANSITION.to_string()));
        }
        let updated = PurchaseOrder {
            status: PurchaseOrderStatus::Purchased,
            ..purchase_order
        };
        let entity = self.gateway.persist(updated).await.map_err(database_error)?;
        let saved = PurchaseOrderEntityMapper::from_active_model(entity);

        if let Some(item_id) = saved.work_order_item_id {
            self.work_orders.resolve_purchase_pendency(item_id).await?;
        }
        Ok(saved)
    }

    /// `TRM-641`: "cancellation available at any point" -- from any status,
    /// including an already-`Cancelled` one (a no-op, the same "terminal
    /// left untouched" shape `WorkOrderUseCase::cancel` already uses).
    /// Deliberately does not touch a linked pendency -- `TRM-647`/`648`
    /// describe only what happens *on receipt*, not on cancellation, and
    /// silently releasing it here could hide a repair that still needs a
    /// part found another way.
    pub async fn cancel(&self, id: i64) -> Result<PurchaseOrder, BusinessError> {
        let purchase_order = self.loaded(id).await?;
        if purchase_order.status == PurchaseOrderStatus::Cancelled {
            return Ok(purchase_order);
        }
        let updated = PurchaseOrder {
            status: PurchaseOrderStatus::Cancelled,
            ..purchase_order
        };
        let entity = self.gateway.persist(updated).await.map_err(database_error)?;
        Ok(PurchaseOrderEntityMapper::from_active_model(entity))
    }

    async fn loaded(&self, id: i64) -> Result<PurchaseOrder, BusinessError> {
        let model = self
            .gateway
            .find_by_id(id)
            .await
            .map_err(database_error)?
            .ok_or_else(|| BusinessError::new(PURCHASE_ORDER_NOT_FOUND.to_string()))?;
        Ok(PurchaseOrderEntityMapper::from_model(model))
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[PurchaseOrderUseCase] {}", msg);
    BusinessError::new(msg)
}
