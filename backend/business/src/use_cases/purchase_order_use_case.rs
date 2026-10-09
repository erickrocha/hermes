use crate::domain::business_error::BusinessError;
use crate::domain::enums::{PurchaseOrderStatus, WorkOrderItemStatus};
use crate::domain::purchase_order::PurchaseOrder;
use crate::domain::work_order_item::WorkOrderItem;
use crate::gateway::purchase_order_gateway::PurchaseOrderStore;
use crate::use_cases::work_order_use_case::WorkOrderUseCase;
use chrono::NaiveDate;
use sea_orm::prelude::async_trait::async_trait;

pub const PART_NOT_FOUND: &str = "Part not found";
pub const PURCHASE_ORDER_NOT_FOUND: &str = "Purchase order not found";
/// `TRM-640`: a purchase order is always for a positive quantity.
pub const QUANTITY_MUST_BE_POSITIVE: &str = "A purchase order quantity must be positive";
/// `TRM-642`: "refuse to create a second active purchase order for the same
/// ... part on the same work order."
pub const DUPLICATE_ACTIVE_PURCHASE_ORDER: &str =
    "This work order already has an active purchase order for this part";
/// `TRM-641`: the state machine only moves Requested -> Ordered -> Purchased.
pub const WRONG_STATUS_FOR_TRANSITION: &str =
    "This purchase order is not in the right status for that transition";
/// `TRM-644`: a pendency is always a pendency *of* a work order.
pub const PENDENCY_REQUIRES_WORK_ORDER: &str =
    "A pendency may only be named together with its own work order";

#[async_trait]
pub trait PurchaseOrderWorkOrderService: Send + Sync {
    /// Checks, before anything is written, that `item_id` is a pendency of `work_order_id`. Raising an
    /// order saves it first and links the pendency after, so a bad pendency found only at the linking
    /// step used to leave the order behind. The default accepts, for services that cannot look.
    async fn check_pendency(
        &self,
        _work_order_id: i64,
        _item_id: i64,
    ) -> Result<(), BusinessError> {
        Ok(())
    }
    async fn add_item(
        &self,
        work_order_id: i64,
        item: WorkOrderItem,
    ) -> Result<(crate::domain::work_order::WorkOrder, WorkOrderItem), BusinessError>;
    async fn mark_item_awaiting_parts(
        &self,
        work_order_id: i64,
        item_id: i64,
        purchase_order_id: i64,
    ) -> Result<crate::domain::work_order::WorkOrder, BusinessError>;
    async fn resolve_purchase_pendency(
        &self,
        item_id: i64,
    ) -> Result<crate::domain::work_order::WorkOrder, BusinessError>;
}

pub struct PurchaseOrderUseCase {
    gateway: Box<dyn PurchaseOrderStore>,
    work_orders: Box<dyn PurchaseOrderWorkOrderService>,
}

impl PurchaseOrderUseCase {
    pub fn new<S, W>(gateway: S, work_orders: W) -> Self
    where
        S: PurchaseOrderStore + 'static,
        W: PurchaseOrderWorkOrderService + 'static,
    {
        Self {
            gateway: Box::new(gateway),
            work_orders: Box::new(work_orders),
        }
    }

    /// `TRM-640…646`: the purchase order itself, and -- when it is raised
    /// against a work order -- the pendency wiring `TRM-644`/`645` describe.
    /// Naming an existing `work_order_item_id` links that pendency
    /// (`TRM-644`); naming none creates a synthetic placeholder pendency
    /// that represents the purchase itself (`TRM-645`). Either way the work
    /// order's own status is recomputed to `AwaitingParts` (`TRM-646`) by
    /// the same `WorkOrderUseCase` machinery `add_item` already uses.
    pub async fn create(
        &self,
        purchase_order: PurchaseOrder,
    ) -> Result<PurchaseOrder, BusinessError> {
        if purchase_order.quantity <= 0.0 {
            return Err(BusinessError::new(QUANTITY_MUST_BE_POSITIVE.to_string()));
        }
        if purchase_order.work_order_item_id.is_some() && purchase_order.work_order_id.is_none() {
            return Err(BusinessError::new(PENDENCY_REQUIRES_WORK_ORDER.to_string()));
        }
        let part = self
            .gateway
            .find_part(purchase_order.part_id)
            .await?
            .ok_or_else(|| BusinessError::new(PART_NOT_FOUND.to_string()))?;
        if let (Some(work_order_id), Some(item_id)) = (
            purchase_order.work_order_id,
            purchase_order.work_order_item_id,
        ) {
            self.work_orders
                .check_pendency(work_order_id, item_id)
                .await?;
        }
        if let Some(work_order_id) = purchase_order.work_order_id {
            let existing = self
                .gateway
                .find_active_by_part_and_work_order(purchase_order.part_id, work_order_id)
                .await?;
            if !existing.is_empty() {
                return Err(BusinessError::new(
                    DUPLICATE_ACTIVE_PURCHASE_ORDER.to_string(),
                ));
            }
        }

        let named_item_id = purchase_order.work_order_item_id;
        let mut saved = self
            .gateway
            .save(PurchaseOrder {
                work_order_item_id: None,
                ..purchase_order
            })
            .await?;

        if let Some(work_order_id) = saved.work_order_id {
            let item_id = match named_item_id {
                Some(item_id) => {
                    self.work_orders
                        .mark_item_awaiting_parts(
                            work_order_id,
                            item_id,
                            saved.id.unwrap_or_default(),
                        )
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
                        preventive_plan_id: None,
                        created_at: None,
                        created_by: None,
                        updated_at: None,
                        updated_by: None,
                    };
                    let (_, saved_item) = self
                        .work_orders
                        .add_item(work_order_id, placeholder)
                        .await?;
                    saved_item.id.unwrap_or_default()
                }
            };

            saved = self
                .gateway
                .save(PurchaseOrder {
                    work_order_item_id: Some(item_id),
                    ..saved
                })
                .await?;
        }

        Ok(saved)
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<PurchaseOrder, BusinessError> {
        match self.gateway.find_by_uuid(uuid).await? {
            Some(order) => Ok(order),
            None => Err(BusinessError::new(PURCHASE_ORDER_NOT_FOUND.to_string())),
        }
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<PurchaseOrder>, u64), BusinessError> {
        self.gateway.find_page(page, page_size).await
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
        self.gateway.save(updated).await
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
        let saved = self.gateway.save(updated).await?;

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
        self.gateway.save(updated).await
    }

    async fn loaded(&self, id: i64) -> Result<PurchaseOrder, BusinessError> {
        let order = self
            .gateway
            .find_by_id(id)
            .await?
            .ok_or_else(|| BusinessError::new(PURCHASE_ORDER_NOT_FOUND.to_string()))?;
        Ok(order)
    }
}

#[async_trait]
impl PurchaseOrderWorkOrderService for WorkOrderUseCase {
    async fn check_pendency(&self, work_order_id: i64, item_id: i64) -> Result<(), BusinessError> {
        self.loaded(work_order_id).await?;
        let item = self.loaded_item(item_id).await?;
        if item.work_order_id != work_order_id {
            return Err(BusinessError::new(
                crate::use_cases::work_order_use_case::ITEM_NOT_ON_THIS_WORK_ORDER.to_string(),
            ));
        }
        Ok(())
    }

    async fn add_item(
        &self,
        work_order_id: i64,
        item: WorkOrderItem,
    ) -> Result<(crate::domain::work_order::WorkOrder, WorkOrderItem), BusinessError> {
        WorkOrderUseCase::add_item(self, work_order_id, item).await
    }

    async fn mark_item_awaiting_parts(
        &self,
        work_order_id: i64,
        item_id: i64,
        purchase_order_id: i64,
    ) -> Result<crate::domain::work_order::WorkOrder, BusinessError> {
        WorkOrderUseCase::mark_item_awaiting_parts(self, work_order_id, item_id, purchase_order_id)
            .await
    }

    async fn resolve_purchase_pendency(
        &self,
        item_id: i64,
    ) -> Result<crate::domain::work_order::WorkOrder, BusinessError> {
        WorkOrderUseCase::resolve_purchase_pendency(self, item_id).await
    }
}
