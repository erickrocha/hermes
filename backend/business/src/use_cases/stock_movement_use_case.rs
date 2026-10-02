use crate::domain::business_error::BusinessError;
use crate::domain::enums::{CostSource, StockMovementType};
use crate::domain::part::Part;
use crate::domain::stock_movement::StockMovement;
use crate::gateway::stock_movement_gateway::StockMovementStore;
use chrono::NaiveDate;
use sea_orm::prelude::async_trait::async_trait;

pub const PART_NOT_FOUND: &str = "Part not found";
/// `TRM-603`: an entry is always a positive quantity; the sign that makes it
/// additive is applied by the ledger, not stated by the caller.
pub const QUANTITY_MUST_BE_POSITIVE: &str = "An entry quantity must be positive";
/// `TRM-611`: a unit value or a total value, never neither.
pub const VALUE_REQUIRED: &str = "An entry needs either a unit value or a total value";
/// `TRM-617`: "accept only a finite non-negative balance."
pub const NON_NEGATIVE_BALANCE_REQUIRED: &str =
    "An adjustment balance must be finite and non-negative";

pub struct StockMovementUseCase {
    store: Box<dyn StockMovementStore>,
}

#[async_trait]
pub trait StockBalanceReader: Send + Sync {
    async fn current_stock(&self, part_id: i64) -> Result<f64, BusinessError>;
}

impl StockMovementUseCase {
    pub fn new<S: StockMovementStore + 'static>(store: S) -> Self {
        Self {
            store: Box::new(store),
        }
    }

    /// `TRM-602`: current stock is the ledger's own sum, never a stored
    /// figure -- see `entity::part_entity`'s doc comment.
    pub async fn current_stock(&self, part_id: i64) -> Result<f64, BusinessError> {
        self.store.current_stock(part_id).await
    }

    /// `TRM-609…612`: records a valued entry and updates the part's moving
    /// average cost and last purchase price in the same transaction
    /// (`TRM-606`'s "mark the part for persistence... independently of the
    /// caller doing so," generalised the same way `EPIC-CK-03-S01` generalised
    /// its own transactional write). `TRM-611`: either `unit_value_cents` or
    /// `total_value_cents` is accepted; the other is derived from `quantity`.
    #[allow(clippy::too_many_arguments)]
    pub async fn record_entry(
        &self,
        part_id: i64,
        quantity: f64,
        unit_value_cents: Option<i64>,
        total_value_cents: Option<i64>,
        supplier: Option<String>,
        invoice_number: Option<String>,
        entry_date: Option<NaiveDate>,
    ) -> Result<StockMovement, BusinessError> {
        if quantity <= 0.0 {
            return Err(BusinessError::new(QUANTITY_MUST_BE_POSITIVE.to_string()));
        }
        let (unit_cents, total_cents) = match (unit_value_cents, total_value_cents) {
            (Some(unit), _) => (unit, (unit as f64 * quantity).round() as i64),
            (None, Some(total)) => (((total as f64) / quantity).round() as i64, total),
            (None, None) => return Err(BusinessError::new(VALUE_REQUIRED.to_string())),
        };

        let part = self
            .store
            .find_part(part_id)
            .await?
            .ok_or_else(|| BusinessError::new(PART_NOT_FOUND.to_string()))?;
        let previous_stock = self.current_stock(part_id).await?;

        let new_average_cents = match part.moving_average_cost_cents {
            Some(previous_average) if previous_stock > 0.0 => {
                let weighted = (previous_average as f64 * previous_stock
                    + unit_cents as f64 * quantity)
                    / (previous_stock + quantity);
                weighted.round() as i64
            }
            _ => unit_cents,
        };

        let movement = StockMovement {
            id: None,
            uuid: None,
            tenant_id: part.tenant_id,
            part_id,
            movement_type: StockMovementType::Entry,
            quantity,
            unit_value_cents: Some(unit_cents),
            total_value_cents: Some(total_cents),
            cost_source: CostSource::Informed,
            supplier,
            invoice_number,
            entry_date,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        };
        let updated_part = Part {
            moving_average_cost_cents: Some(new_average_cents),
            last_purchase_price_cents: Some(unit_cents),
            ..part
        };

        self.store
            .record_entry_and_update_part(movement, updated_part)
            .await
    }

    /// `TRM-604`/`617`: reconciles the part to `new_balance` with a single
    /// explicit adjustment movement for the difference -- never a second,
    /// competing figure. `TRM-605`'s "abort when the movement store has not
    /// loaded" is a legacy client-side blob concern; a `SUM` over a
    /// relational table has no "unloaded" state to guard against.
    pub async fn adjust(
        &self,
        part_id: i64,
        new_balance: f64,
    ) -> Result<StockMovement, BusinessError> {
        if !new_balance.is_finite() || new_balance < 0.0 {
            return Err(BusinessError::new(
                NON_NEGATIVE_BALANCE_REQUIRED.to_string(),
            ));
        }
        let part = self
            .store
            .find_part(part_id)
            .await?
            .ok_or_else(|| BusinessError::new(PART_NOT_FOUND.to_string()))?;
        let current = self.current_stock(part_id).await?;

        let movement = StockMovement {
            id: None,
            uuid: None,
            tenant_id: part.tenant_id,
            part_id,
            movement_type: StockMovementType::Adjustment,
            quantity: new_balance - current,
            unit_value_cents: None,
            total_value_cents: None,
            cost_source: CostSource::None,
            supplier: None,
            invoice_number: None,
            entry_date: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        };
        self.store.save_adjustment(movement).await
    }
}

#[async_trait]
impl StockBalanceReader for StockMovementUseCase {
    async fn current_stock(&self, part_id: i64) -> Result<f64, BusinessError> {
        StockMovementUseCase::current_stock(self, part_id).await
    }
}
