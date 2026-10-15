use business::domain::business_error::BusinessError;
use business::domain::enums::{CostSource, StockMovementType};
use business::domain::part::Part;
use business::domain::stock_movement::StockMovement;
use business::gateway::stock_movement_gateway::StockMovementStore;
use business::use_cases::stock_movement_use_case::StockMovementUseCase;
use sea_orm::prelude::async_trait::async_trait;
use std::sync::{Arc, Mutex};

struct MemoryStockMovementStore {
    part: Arc<Mutex<Option<Part>>>,
    stock: Mutex<f64>,
    movements: Mutex<Vec<StockMovement>>,
}

impl MemoryStockMovementStore {
    fn new(part: Part, stock: f64) -> (Self, Arc<Mutex<Option<Part>>>) {
        let part = Arc::new(Mutex::new(Some(part)));
        (
            Self {
                part: Arc::clone(&part),
                stock: Mutex::new(stock),
                movements: Mutex::new(Vec::new()),
            },
            part,
        )
    }
}

#[async_trait]
impl StockMovementStore for MemoryStockMovementStore {
    async fn find_part(&self, part_id: i64) -> Result<Option<Part>, BusinessError> {
        let part = self
            .part
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(part
            .as_ref()
            .filter(|part| part.id == Some(part_id))
            .cloned())
    }

    async fn current_stock(&self, _part_id: i64) -> Result<f64, BusinessError> {
        let stock = self
            .stock
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(*stock)
    }

    async fn save_adjustment(
        &self,
        movement: StockMovement,
    ) -> Result<StockMovement, BusinessError> {
        *self
            .stock
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))? += movement.quantity;
        self.movements
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?
            .push(movement.clone());
        Ok(movement)
    }

    async fn record_entry_and_update_part(
        &self,
        movement: StockMovement,
        part: Part,
    ) -> Result<StockMovement, BusinessError> {
        *self
            .part
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))? = Some(part);
        *self
            .stock
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))? += movement.quantity;
        self.movements
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?
            .push(movement.clone());
        Ok(movement)
    }
}

fn part() -> Part {
    Part {
        id: Some(1),
        uuid: Some("part-1".into()),
        tenant_id: Some(42),
        name: "Brake pad".into(),
        category: None,
        application: None,
        minimum_stock: 0.0,
        unit: "unit".into(),
        unit_value_cents: None,
        default_supplier: None,
        location: None,
        observation: None,
        moving_average_cost_cents: Some(10_000),
        last_purchase_price_cents: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn an_entry_recalculates_the_weighted_average_and_last_purchase_price() {
    let (store, persisted_part) = MemoryStockMovementStore::new(part(), 10.0);
    let use_case = StockMovementUseCase::new(store);

    let movement = use_case
        .record_entry(1, 2.0, Some(15_000), None, None, None, None)
        .await
        .expect("entry is recorded");

    assert_eq!(movement.unit_value_cents, Some(15_000));
    assert_eq!(movement.total_value_cents, Some(30_000));
    let part = persisted_part
        .lock()
        .expect("memory store remains available")
        .clone()
        .expect("part remains stored");
    assert_eq!(part.moving_average_cost_cents, Some(10_833));
    assert_eq!(part.last_purchase_price_cents, Some(15_000));
}

#[tokio::test]
async fn an_adjustment_records_only_the_difference_from_ledger_stock() {
    let (store, _) = MemoryStockMovementStore::new(part(), 7.5);
    let use_case = StockMovementUseCase::new(store);

    let movement = use_case.adjust(1, 9.0).await.expect("adjustment is saved");

    assert_eq!(movement.movement_type, StockMovementType::Adjustment);
    assert_eq!(movement.quantity, 1.5);
    assert_eq!(movement.cost_source, CostSource::None);
}
