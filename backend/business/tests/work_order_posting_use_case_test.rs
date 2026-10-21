use business::domain::business_error::BusinessError;
use business::domain::enums::{
    CostSource, StockMovementType, WorkOrderItemStatus, WorkOrderOrigin, WorkOrderStatus,
};
use business::domain::part::Part;
use business::domain::stock_movement::StockMovement;
use business::domain::work_order::WorkOrder;
use business::domain::work_order_item::WorkOrderItem;
use business::domain::work_order_posting::WorkOrderPosting;
use business::gateway::work_order_posting_gateway::WorkOrderPostingStore;
use business::use_cases::stock_movement_use_case::StockBalanceReader;
use business::use_cases::work_order_posting_use_case::{
    INSUFFICIENT_STOCK, ITEM_NOT_ON_THIS_WORK_ORDER, PART_NOT_FOUND, QUANTITY_MUST_BE_POSITIVE,
    WORK_ORDER_NOT_FOUND, WorkOrderPostingUseCase,
};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

struct MemoryPostingStore {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl WorkOrderPostingStore for MemoryPostingStore {
    async fn find_work_order(&self, _id: i64) -> Result<Option<WorkOrder>, BusinessError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(None)
    }

    async fn find_work_order_item(&self, _id: i64) -> Result<Option<WorkOrderItem>, BusinessError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(None)
    }

    async fn find_part(&self, _id: i64) -> Result<Option<Part>, BusinessError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(None)
    }

    async fn issue_atomically(
        &self,
        _movement: StockMovement,
        _posting: WorkOrderPosting,
    ) -> Result<WorkOrderPosting, BusinessError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Err(BusinessError::new("unexpected issue write".into()))
    }

    async fn list_by_work_order(
        &self,
        _work_order_id: i64,
    ) -> Result<Vec<WorkOrderPosting>, BusinessError> {
        Ok(Vec::new())
    }
}

struct MemoryStockBalance {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl StockBalanceReader for MemoryStockBalance {
    async fn current_stock(&self, _part_id: i64) -> Result<f64, BusinessError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(0.0)
    }
}

#[tokio::test]
async fn a_non_positive_issue_is_rejected_before_any_lookup_or_stock_read() {
    let store_calls = Arc::new(AtomicUsize::new(0));
    let stock_calls = Arc::new(AtomicUsize::new(0));
    let use_case = WorkOrderPostingUseCase::new(
        MemoryPostingStore {
            calls: Arc::clone(&store_calls),
        },
        MemoryStockBalance {
            calls: Arc::clone(&stock_calls),
        },
    );

    let error = use_case
        .issue(1, None, 2, 0.0)
        .await
        .expect_err("issue quantity must be positive");

    assert_eq!(error.message, QUANTITY_MUST_BE_POSITIVE);
    assert_eq!(store_calls.load(Ordering::Relaxed), 0);
    assert_eq!(stock_calls.load(Ordering::Relaxed), 0);
}

#[derive(Clone)]
struct ScenarioPostingStore {
    work_order: Option<WorkOrder>,
    item: Option<WorkOrderItem>,
    part: Option<Part>,
    postings: Vec<WorkOrderPosting>,
    movement: Arc<Mutex<Option<StockMovement>>>,
}

#[async_trait]
impl WorkOrderPostingStore for ScenarioPostingStore {
    async fn find_work_order(&self, id: i64) -> Result<Option<WorkOrder>, BusinessError> {
        Ok(self
            .work_order
            .as_ref()
            .filter(|row| row.id == Some(id))
            .cloned())
    }

    async fn find_work_order_item(&self, id: i64) -> Result<Option<WorkOrderItem>, BusinessError> {
        Ok(self.item.as_ref().filter(|row| row.id == Some(id)).cloned())
    }

    async fn find_part(&self, id: i64) -> Result<Option<Part>, BusinessError> {
        Ok(self.part.as_ref().filter(|row| row.id == Some(id)).cloned())
    }

    async fn issue_atomically(
        &self,
        movement: StockMovement,
        mut posting: WorkOrderPosting,
    ) -> Result<WorkOrderPosting, BusinessError> {
        *self.movement.lock().unwrap() = Some(movement);
        posting.id = Some(500);
        posting.uuid = Some("posting-500".into());
        Ok(posting)
    }

    async fn list_by_work_order(
        &self,
        work_order_id: i64,
    ) -> Result<Vec<WorkOrderPosting>, BusinessError> {
        Ok(self
            .postings
            .iter()
            .filter(|posting| posting.work_order_id == work_order_id)
            .cloned()
            .collect())
    }
}

struct FixedStock(f64);

#[async_trait]
impl StockBalanceReader for FixedStock {
    async fn current_stock(&self, _part_id: i64) -> Result<f64, BusinessError> {
        Ok(self.0)
    }
}

fn work_order(tenant_id: Option<i64>) -> WorkOrder {
    WorkOrder {
        id: Some(94),
        uuid: Some("work-order-94".into()),
        tenant_id,
        vehicle_id: 10,
        opened_at: None,
        odometer_km: 100.0,
        origin: WorkOrderOrigin::Manual,
        checklist_run_id: None,
        maintenance_plan_id: None,
        preventive_plan_id: None,
        service_type: None,
        description: "Repair".into(),
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
    }
}

fn work_order_item(work_order_id: i64) -> WorkOrderItem {
    WorkOrderItem {
        id: Some(97),
        uuid: Some("item-97".into()),
        tenant_id: Some(42),
        work_order_id,
        description: "Brake pad".into(),
        item_type: None,
        status: WorkOrderItemStatus::Pending,
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

fn part(
    moving_average_cost_cents: Option<i64>,
    last_purchase_price_cents: Option<i64>,
    unit_value_cents: Option<i64>,
    tenant_id: Option<i64>,
) -> Part {
    Part {
        id: Some(2),
        uuid: Some("part-2".into()),
        tenant_id,
        name: "Brake pad".into(),
        category: None,
        application: None,
        minimum_stock: 0.0,
        unit: "unit".into(),
        unit_value_cents,
        default_supplier: None,
        location: None,
        observation: None,
        moving_average_cost_cents,
        last_purchase_price_cents,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn configured_use_case(
    work_order: Option<WorkOrder>,
    item: Option<WorkOrderItem>,
    part: Option<Part>,
    stock: f64,
    postings: Vec<WorkOrderPosting>,
) -> (WorkOrderPostingUseCase, Arc<Mutex<Option<StockMovement>>>) {
    let movement = Arc::new(Mutex::new(None));
    (
        WorkOrderPostingUseCase::new(
            ScenarioPostingStore {
                work_order,
                item,
                part,
                postings,
                movement: Arc::clone(&movement),
            },
            FixedStock(stock),
        ),
        movement,
    )
}

#[tokio::test]
async fn an_issue_checks_work_order_item_part_and_stock_before_writing() {
    let (missing_order, _) = configured_use_case(
        None,
        Some(work_order_item(94)),
        Some(part(None, None, None, Some(42))),
        10.0,
        vec![],
    );
    assert_eq!(
        missing_order
            .issue(94, None, 2, 1.0)
            .await
            .unwrap_err()
            .message,
        WORK_ORDER_NOT_FOUND
    );

    let (missing_item, _) = configured_use_case(
        Some(work_order(Some(42))),
        None,
        Some(part(None, None, None, Some(42))),
        10.0,
        vec![],
    );
    assert_eq!(
        missing_item
            .issue(94, Some(97), 2, 1.0)
            .await
            .unwrap_err()
            .message,
        ITEM_NOT_ON_THIS_WORK_ORDER
    );

    let (wrong_order_item, _) = configured_use_case(
        Some(work_order(Some(42))),
        Some(work_order_item(95)),
        Some(part(None, None, None, Some(42))),
        10.0,
        vec![],
    );
    assert_eq!(
        wrong_order_item
            .issue(94, Some(97), 2, 1.0)
            .await
            .unwrap_err()
            .message,
        ITEM_NOT_ON_THIS_WORK_ORDER
    );

    let (missing_part, _) =
        configured_use_case(Some(work_order(Some(42))), None, None, 10.0, vec![]);
    assert_eq!(
        missing_part
            .issue(94, None, 2, 1.0)
            .await
            .unwrap_err()
            .message,
        PART_NOT_FOUND
    );

    let (foreign_part, _) = configured_use_case(
        Some(work_order(Some(42))),
        None,
        Some(part(None, None, None, Some(99))),
        10.0,
        vec![],
    );
    assert_eq!(
        foreign_part
            .issue(94, None, 2, 1.0)
            .await
            .unwrap_err()
            .message,
        PART_NOT_FOUND
    );

    let (insufficient, _) = configured_use_case(
        Some(work_order(Some(42))),
        None,
        Some(part(None, None, None, Some(42))),
        0.5,
        vec![],
    );
    assert_eq!(
        insufficient
            .issue(94, None, 2, 1.0)
            .await
            .unwrap_err()
            .message,
        INSUFFICIENT_STOCK
    );
}

#[tokio::test]
async fn issuing_records_negative_stock_movement_and_uses_cost_precedence() {
    let cases = [
        (
            Some(700),
            Some(600),
            Some(500),
            700,
            CostSource::MovingAverageCost,
        ),
        (
            None,
            Some(600),
            Some(500),
            600,
            CostSource::LastPurchasePrice,
        ),
        (None, None, Some(500), 500, CostSource::RegisteredUnitValue),
        (None, None, None, 0, CostSource::None),
    ];
    for (average, last, unit, expected, source) in cases {
        let (use_case, movement) = configured_use_case(
            Some(work_order(Some(42))),
            Some(work_order_item(94)),
            Some(part(average, last, unit, Some(42))),
            5.0,
            vec![],
        );
        let posting = use_case
            .issue(94, Some(97), 2, 2.5)
            .await
            .expect("issue is written atomically");

        assert_eq!(posting.unit_value_cents, expected);
        assert_eq!(
            posting.total_value_cents,
            (expected as f64 * 2.5).round() as i64
        );
        assert_eq!(posting.cost_source, source);
        let movement = movement.lock().unwrap().clone().unwrap();
        assert_eq!(movement.movement_type, StockMovementType::Issue);
        assert_eq!(movement.quantity, -2.5);
        assert_eq!(movement.total_value_cents, Some(posting.total_value_cents));
    }
}

#[tokio::test]
async fn posting_history_and_totals_are_scoped_to_the_work_order() {
    let posting = WorkOrderPosting {
        id: Some(500),
        uuid: Some("posting-500".into()),
        tenant_id: Some(42),
        work_order_id: 94,
        work_order_item_id: Some(97),
        part_id: 2,
        quantity: 2.0,
        unit_value_cents: 350,
        total_value_cents: 700,
        cost_source: CostSource::MovingAverageCost,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    };
    let (use_case, _) = configured_use_case(
        Some(work_order(Some(42))),
        None,
        Some(part(Some(350), None, None, Some(42))),
        10.0,
        vec![posting.clone()],
    );

    assert_eq!(
        use_case.find_by_work_order(94).await.unwrap(),
        vec![posting]
    );
    assert_eq!(use_case.postings_total_cents(94).await.unwrap(), 700);
    assert_eq!(use_case.find_by_work_order(95).await.unwrap(), vec![]);
}
