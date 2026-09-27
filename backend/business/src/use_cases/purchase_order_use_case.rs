use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::PurchaseOrderStatus;
use crate::domain::purchase_order::{PurchaseOrder, PurchaseOrderEntityMapper};
use crate::gateway::part_gateway::PartGateway;
use crate::gateway::purchase_order_gateway::PurchaseOrderGateway;
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

pub struct PurchaseOrderUseCase {
    gateway: PurchaseOrderGateway,
    parts: PartGateway,
}

impl PurchaseOrderUseCase {
    pub fn new(gateway: PurchaseOrderGateway, parts: PartGateway) -> Self {
        Self { gateway, parts }
    }

    /// `TRM-640…643`: the purchase order itself. `TRM-644…650` (pendency
    /// wiring) is `EPIC-SP-03-S02`'s own scope, built the day it exists.
    pub async fn create(&self, purchase_order: PurchaseOrder) -> Result<PurchaseOrder, BusinessError> {
        if purchase_order.quantity <= 0.0 {
            return Err(BusinessError::new(QUANTITY_MUST_BE_POSITIVE.to_string()));
        }
        let part_exists = self
            .parts
            .find_by_id(purchase_order.part_id)
            .await
            .map_err(database_error)?
            .is_some();
        if !part_exists {
            return Err(BusinessError::new(PART_NOT_FOUND.to_string()));
        }
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

        let entity = self.gateway.persist(purchase_order).await.map_err(database_error)?;
        Ok(PurchaseOrderEntityMapper::from_active_model(entity))
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

    /// `TRM-641`: `Ordered` -> `Purchased`.
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
        Ok(PurchaseOrderEntityMapper::from_active_model(entity))
    }

    /// `TRM-641`: "cancellation available at any point" -- from any status,
    /// including an already-`Cancelled` one (a no-op, the same "terminal
    /// left untouched" shape `WorkOrderUseCase::cancel` already uses).
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
