use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::{CostSource, StockMovementType};
use crate::domain::part::{Part, PartEntityMapper};
use crate::domain::stock_movement::{StockMovement, StockMovementEntityMapper};
use crate::gateway::part_gateway::PartGateway;
use crate::gateway::stock_movement_gateway::StockMovementGateway;
use chrono::NaiveDate;
use sea_orm::{ActiveModelTrait, DbErr, TransactionTrait};

pub const PART_NOT_FOUND: &str = "Part not found";
/// `TRM-603`: an entry is always a positive quantity; the sign that makes it
/// additive is applied by the ledger, not stated by the caller.
pub const QUANTITY_MUST_BE_POSITIVE: &str = "An entry quantity must be positive";
/// `TRM-611`: a unit value or a total value, never neither.
pub const VALUE_REQUIRED: &str = "An entry needs either a unit value or a total value";
/// `TRM-617`: "accept only a finite non-negative balance."
pub const NON_NEGATIVE_BALANCE_REQUIRED: &str = "An adjustment balance must be finite and non-negative";

pub struct StockMovementUseCase {
    movements: StockMovementGateway,
    parts: PartGateway,
}

impl StockMovementUseCase {
    pub fn new(movements: StockMovementGateway, parts: PartGateway) -> Self {
        Self { movements, parts }
    }

    /// `TRM-602`: current stock is the ledger's own sum, never a stored
    /// figure -- see `entity::part_entity`'s doc comment.
    pub async fn current_stock(&self, part_id: i64) -> Result<f64, BusinessError> {
        let rows = self.movements.find_by_part(part_id).await.map_err(database_error)?;
        Ok(rows.iter().map(|m| m.quantity).sum())
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

        let part_model = self
            .parts
            .find_by_id(part_id)
            .await
            .map_err(database_error)?
            .ok_or_else(|| BusinessError::new(PART_NOT_FOUND.to_string()))?;
        let part = PartEntityMapper::from_model(part_model);
        let previous_stock = self.current_stock(part_id).await?;

        let new_average_cents = match part.moving_average_cost_cents {
            Some(previous_average) if previous_stock > 0.0 => {
                let weighted = (previous_average as f64 * previous_stock + unit_cents as f64 * quantity)
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

        let transaction = self.parts.db().begin().await.map_err(database_error)?;
        let saved_movement = match StockMovementEntityMapper::build_active_model(movement)
            .save(&transaction)
            .await
        {
            Ok(active) => active,
            Err(e) => {
                let _ = transaction.rollback().await;
                return Err(database_error(e));
            }
        };
        if let Err(e) = PartEntityMapper::build_active_model(updated_part)
            .save(&transaction)
            .await
        {
            let _ = transaction.rollback().await;
            return Err(database_error(e));
        }
        transaction.commit().await.map_err(database_error)?;

        Ok(StockMovementEntityMapper::from_active_model(saved_movement))
    }

    /// `TRM-604`/`617`: reconciles the part to `new_balance` with a single
    /// explicit adjustment movement for the difference -- never a second,
    /// competing figure. `TRM-605`'s "abort when the movement store has not
    /// loaded" is a legacy client-side blob concern; a `SUM` over a
    /// relational table has no "unloaded" state to guard against.
    pub async fn adjust(&self, part_id: i64, new_balance: f64) -> Result<StockMovement, BusinessError> {
        if !new_balance.is_finite() || new_balance < 0.0 {
            return Err(BusinessError::new(NON_NEGATIVE_BALANCE_REQUIRED.to_string()));
        }
        let part_model = self
            .parts
            .find_by_id(part_id)
            .await
            .map_err(database_error)?
            .ok_or_else(|| BusinessError::new(PART_NOT_FOUND.to_string()))?;
        let current = self.current_stock(part_id).await?;

        let movement = StockMovement {
            id: None,
            uuid: None,
            tenant_id: part_model.tenant_id,
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
        let entity = self.movements.persist(movement).await.map_err(database_error)?;
        Ok(StockMovementEntityMapper::from_active_model(entity))
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[StockMovementUseCase] {}", msg);
    BusinessError::new(msg)
}
