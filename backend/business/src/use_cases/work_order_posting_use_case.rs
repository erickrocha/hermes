use crate::domain::business_error::BusinessError;
use crate::domain::enums::{CostSource, StockMovementType};
use crate::domain::part::Part;
use crate::domain::stock_movement::StockMovement;
use crate::domain::work_order_posting::WorkOrderPosting;
use crate::gateway::work_order_posting_gateway::WorkOrderPostingStore;
use crate::use_cases::stock_movement_use_case::StockBalanceReader;

pub const WORK_ORDER_NOT_FOUND: &str = "Work order not found";
pub const ITEM_NOT_ON_THIS_WORK_ORDER: &str = "This item does not belong to the named work order";
pub const PART_NOT_FOUND: &str = "Part not found";
/// `TRM-640`-shaped rule applied here too: an issue is always a positive
/// quantity.
pub const QUANTITY_MUST_BE_POSITIVE: &str = "An issued quantity must be positive";
/// `TRM-613`: "refuse to issue a part to a work order when the recorded
/// stock is below the quantity requested, stating the stock and its unit."
pub const INSUFFICIENT_STOCK: &str = "Insufficient stock to issue this quantity";

pub struct WorkOrderPostingUseCase {
    store: Box<dyn WorkOrderPostingStore>,
    stock: Box<dyn StockBalanceReader>,
}

impl WorkOrderPostingUseCase {
    pub fn new<S, B>(store: S, stock: B) -> Self
    where
        S: WorkOrderPostingStore + 'static,
        B: StockBalanceReader + 'static,
    {
        Self {
            store: Box::new(store),
            stock: Box::new(stock),
        }
    }

    /// `TRM-609`/`613`/`614`: issuing a part to a work order is a stock
    /// movement (`TRM-613`'s refusal, `TRM-609`'s cost-source stamp) *and* a
    /// costed posting (`TRM-614`) in one transaction -- the same shape
    /// `StockMovementUseCase::record_entry` already uses for its own
    /// part-plus-movement write. Unlike `record_entry`, an issue never
    /// updates the part's moving average or last purchase price: `TRM-610`
    /// updates cost only "on every valued entry," and an issue consumes
    /// stock, it does not value it.
    pub async fn issue(
        &self,
        work_order_id: i64,
        work_order_item_id: Option<i64>,
        part_id: i64,
        quantity: f64,
    ) -> Result<WorkOrderPosting, BusinessError> {
        if quantity <= 0.0 {
            return Err(BusinessError::new(QUANTITY_MUST_BE_POSITIVE.to_string()));
        }

        let work_order = self
            .store
            .find_work_order(work_order_id)
            .await?
            .ok_or_else(|| BusinessError::new(WORK_ORDER_NOT_FOUND.to_string()))?;

        if let Some(item_id) = work_order_item_id {
            let item = self
                .store
                .find_work_order_item(item_id)
                .await?
                .ok_or_else(|| BusinessError::new(ITEM_NOT_ON_THIS_WORK_ORDER.to_string()))?;
            if item.work_order_id != work_order_id {
                return Err(BusinessError::new(ITEM_NOT_ON_THIS_WORK_ORDER.to_string()));
            }
        }

        let part = self
            .store
            .find_part(part_id)
            .await?
            .ok_or_else(|| BusinessError::new(PART_NOT_FOUND.to_string()))?;
        if part.tenant_id != work_order.tenant_id {
            return Err(BusinessError::new(PART_NOT_FOUND.to_string()));
        }

        let current_stock = self.stock.current_stock(part_id).await?;
        if current_stock < quantity {
            return Err(BusinessError::new(INSUFFICIENT_STOCK.to_string()));
        }

        let (unit_value_cents, cost_source) = cost_of(&part);
        let total_value_cents = (unit_value_cents as f64 * quantity).round() as i64;

        let movement = StockMovement {
            id: None,
            uuid: None,
            tenant_id: work_order.tenant_id,
            part_id,
            movement_type: StockMovementType::Issue,
            quantity: -quantity,
            unit_value_cents: Some(unit_value_cents),
            total_value_cents: Some(total_value_cents),
            cost_source: cost_source.clone(),
            supplier: None,
            invoice_number: None,
            entry_date: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        };
        let posting = WorkOrderPosting {
            id: None,
            uuid: None,
            tenant_id: work_order.tenant_id,
            work_order_id,
            work_order_item_id,
            part_id,
            quantity,
            unit_value_cents,
            total_value_cents,
            cost_source,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        };

        self.store.issue_atomically(movement, posting).await
    }

    pub async fn find_by_work_order(
        &self,
        work_order_id: i64,
    ) -> Result<Vec<WorkOrderPosting>, BusinessError> {
        self.store.list_by_work_order(work_order_id).await
    }

    /// `TRM-688`: a work order's postings summed -- the part of its total
    /// cost this epic owns. External-service value and other costs are the
    /// caller's own (`work_order.invoice_value_cents`; "other costs" has no
    /// entity in hermes yet).
    pub async fn postings_total_cents(&self, work_order_id: i64) -> Result<i64, BusinessError> {
        let postings = self.find_by_work_order(work_order_id).await?;
        Ok(postings.iter().map(|p| p.total_value_cents).sum())
    }
}

/// `TRM-608`: moving average cost, then last purchase price, then the
/// registered unit value -- identically to the precedence `EPIC-SP-02-S01`
/// deferred building until it had a caller. No source at all is `CostSource::
/// None` with a zero value, the same "unknown/absent" degrade this program's
/// enums already use elsewhere, rather than refusing the issue outright --
/// TRM-613 only requires refusing on *insufficient stock*, not on an unpriced
/// part.
fn cost_of(part: &Part) -> (i64, CostSource) {
    if let Some(v) = part.moving_average_cost_cents {
        (v, CostSource::MovingAverageCost)
    } else if let Some(v) = part.last_purchase_price_cents {
        (v, CostSource::LastPurchasePrice)
    } else if let Some(v) = part.unit_value_cents {
        (v, CostSource::RegisteredUnitValue)
    } else {
        (0, CostSource::None)
    }
}
