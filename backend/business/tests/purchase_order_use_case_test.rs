use business::domain::business_error::BusinessError;
use business::domain::enums::PurchaseOrderStatus;
use business::domain::part::Part;
use business::domain::purchase_order::PurchaseOrder;
use business::domain::work_order::WorkOrder;
use business::domain::work_order_item::WorkOrderItem;
use business::gateway::purchase_order_gateway::PurchaseOrderStore;
use business::use_cases::purchase_order_use_case::{
    PART_NOT_FOUND, PENDENCY_REQUIRES_WORK_ORDER, PURCHASE_ORDER_NOT_FOUND, PurchaseOrderUseCase,
    PurchaseOrderWorkOrderService, QUANTITY_MUST_BE_POSITIVE, WRONG_STATUS_FOR_TRANSITION,
};
use chrono::NaiveDate;
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct MemoryPurchaseOrderStore {
    reads: Arc<AtomicUsize>,
    loaded_order: Option<PurchaseOrder>,
    part: Option<Part>,
}

#[async_trait]
impl PurchaseOrderStore for MemoryPurchaseOrderStore {
    async fn find_part(&self, _part_id: i64) -> Result<Option<Part>, BusinessError> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        Ok(self.part.clone())
    }

    async fn save(&self, order: PurchaseOrder) -> Result<PurchaseOrder, BusinessError> {
        Ok(order)
    }

    async fn find_by_id(&self, _id: i64) -> Result<Option<PurchaseOrder>, BusinessError> {
        Ok(self.loaded_order.clone())
    }

    async fn find_by_uuid(&self, _uuid: String) -> Result<Option<PurchaseOrder>, BusinessError> {
        Ok(None)
    }

    async fn find_page(
        &self,
        _page: u64,
        _page_size: u64,
    ) -> Result<(Vec<PurchaseOrder>, u64), BusinessError> {
        Ok((Vec::new(), 0))
    }

    async fn find_active_by_part_and_work_order(
        &self,
        _part_id: i64,
        _work_order_id: i64,
    ) -> Result<Vec<PurchaseOrder>, BusinessError> {
        Ok(Vec::new())
    }
}

struct NoWorkOrderOperations;

#[async_trait]
impl PurchaseOrderWorkOrderService for NoWorkOrderOperations {
    async fn add_item(
        &self,
        _work_order_id: i64,
        _item: WorkOrderItem,
    ) -> Result<(WorkOrder, WorkOrderItem), BusinessError> {
        Err(BusinessError::new("unexpected work-order call".into()))
    }

    async fn mark_item_awaiting_parts(
        &self,
        _work_order_id: i64,
        _item_id: i64,
        _purchase_order_id: i64,
    ) -> Result<WorkOrder, BusinessError> {
        Err(BusinessError::new("unexpected work-order call".into()))
    }

    async fn resolve_purchase_pendency(&self, _item_id: i64) -> Result<WorkOrder, BusinessError> {
        Err(BusinessError::new("unexpected work-order call".into()))
    }
}

fn order(quantity: f64) -> PurchaseOrder {
    PurchaseOrder {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        part_id: 7,
        quantity,
        suggested_supplier: None,
        observation: None,
        status: Default::default(),
        work_order_id: None,
        work_order_item_id: None,
        vehicle_id: None,
        ordered_at: None,
        expected_delivery_date: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn part() -> Part {
    Part {
        id: Some(7),
        uuid: Some("part-7".into()),
        tenant_id: Some(42),
        name: "Brake pad".into(),
        category: None,
        application: None,
        minimum_stock: 0.0,
        unit: "unit".into(),
        unit_value_cents: Some(2_500),
        default_supplier: None,
        location: None,
        observation: None,
        moving_average_cost_cents: None,
        last_purchase_price_cents: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn a_non_positive_quantity_is_rejected_before_any_store_or_work_order_call() {
    let reads = Arc::new(AtomicUsize::new(0));
    let store = MemoryPurchaseOrderStore {
        reads: Arc::clone(&reads),
        loaded_order: None,
        part: None,
    };
    let use_case = PurchaseOrderUseCase::new(store, NoWorkOrderOperations);

    let error = use_case
        .create(order(0.0))
        .await
        .expect_err("purchase quantity must be positive");

    assert_eq!(error.message, QUANTITY_MUST_BE_POSITIVE);
    assert_eq!(reads.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn purchase_order_creation_requires_a_part_and_accepts_a_valid_standalone_order() {
    let reads = Arc::new(AtomicUsize::new(0));
    let missing_part = PurchaseOrderUseCase::new(
        MemoryPurchaseOrderStore {
            reads: Arc::clone(&reads),
            loaded_order: None,
            part: None,
        },
        NoWorkOrderOperations,
    );
    assert_eq!(
        missing_part
            .create(order(2.0))
            .await
            .expect_err("unknown part cannot be ordered")
            .message,
        PART_NOT_FOUND
    );

    let standalone = PurchaseOrderUseCase::new(
        MemoryPurchaseOrderStore {
            reads: Arc::clone(&reads),
            loaded_order: None,
            part: Some(part()),
        },
        NoWorkOrderOperations,
    );
    let created = standalone
        .create(order(2.0))
        .await
        .expect("valid part can be ordered without a work order");
    assert_eq!(created.part_id, 7);
    assert_eq!(created.quantity, 2.0);
    assert_eq!(created.work_order_item_id, None);

    let mut unattached_pendency = order(1.0);
    unattached_pendency.work_order_item_id = Some(8);
    assert_eq!(
        standalone
            .create(unattached_pendency)
            .await
            .expect_err("an item must be linked through its work order")
            .message,
        PENDENCY_REQUIRES_WORK_ORDER
    );
}

#[tokio::test]
async fn purchase_order_transitions_enforce_status_and_update_order_dates() {
    let reads = Arc::new(AtomicUsize::new(0));
    let use_case = PurchaseOrderUseCase::new(
        MemoryPurchaseOrderStore {
            reads,
            loaded_order: Some(order(2.0)),
            part: None,
        },
        NoWorkOrderOperations,
    );
    let ordered_at = NaiveDate::from_ymd_opt(2026, 10, 1).expect("valid date");
    let expected_delivery_date = NaiveDate::from_ymd_opt(2026, 10, 5).expect("valid date");

    let ordered = use_case
        .mark_ordered(7, Some(ordered_at), Some(expected_delivery_date))
        .await
        .expect("requested order can be ordered");
    assert_eq!(ordered.status, PurchaseOrderStatus::Ordered);
    assert_eq!(ordered.ordered_at, Some(ordered_at));
    assert_eq!(ordered.expected_delivery_date, Some(expected_delivery_date));

    let error = use_case
        .mark_purchased(7)
        .await
        .expect_err("requested order cannot be received before ordering");
    assert_eq!(error.message, WRONG_STATUS_FOR_TRANSITION);

    let mut already_ordered = order(2.0);
    already_ordered.status = PurchaseOrderStatus::Ordered;
    let already_ordered_use_case = PurchaseOrderUseCase::new(
        MemoryPurchaseOrderStore {
            reads: Arc::new(AtomicUsize::new(0)),
            loaded_order: Some(already_ordered),
            part: None,
        },
        NoWorkOrderOperations,
    );
    let error = already_ordered_use_case
        .mark_ordered(7, None, None)
        .await
        .expect_err("an already ordered order cannot be ordered twice");
    assert_eq!(error.message, WRONG_STATUS_FOR_TRANSITION);
}

#[tokio::test]
async fn purchase_order_receipt_and_cancellation_preserve_terminal_behavior() {
    let reads = Arc::new(AtomicUsize::new(0));
    let mut received_order = order(2.0);
    received_order.status = PurchaseOrderStatus::Ordered;
    let use_case = PurchaseOrderUseCase::new(
        MemoryPurchaseOrderStore {
            reads: Arc::clone(&reads),
            loaded_order: Some(received_order),
            part: None,
        },
        NoWorkOrderOperations,
    );

    let purchased = use_case
        .mark_purchased(7)
        .await
        .expect("ordered order can be received");
    assert_eq!(purchased.status, PurchaseOrderStatus::Purchased);
    let cancelled = use_case.cancel(7).await.expect("order can be cancelled");
    assert_eq!(cancelled.status, PurchaseOrderStatus::Cancelled);

    let mut already_cancelled = order(2.0);
    already_cancelled.status = PurchaseOrderStatus::Cancelled;
    let already_cancelled_use_case = PurchaseOrderUseCase::new(
        MemoryPurchaseOrderStore {
            reads,
            loaded_order: Some(already_cancelled),
            part: None,
        },
        NoWorkOrderOperations,
    );
    assert_eq!(
        already_cancelled_use_case
            .cancel(7)
            .await
            .expect("cancellation is idempotent")
            .status,
        PurchaseOrderStatus::Cancelled
    );
}

#[tokio::test]
async fn purchase_order_queries_report_missing_records_and_delegate_paging() {
    let reads = Arc::new(AtomicUsize::new(0));
    let use_case = PurchaseOrderUseCase::new(
        MemoryPurchaseOrderStore {
            reads,
            loaded_order: None,
            part: None,
        },
        NoWorkOrderOperations,
    );
    assert_eq!(
        use_case
            .find_by_uuid("absent".into())
            .await
            .expect_err("unknown UUID should fail")
            .message,
        PURCHASE_ORDER_NOT_FOUND
    );
    assert_eq!(
        use_case
            .mark_ordered(404, None, None)
            .await
            .expect_err("unknown ID should fail")
            .message,
        PURCHASE_ORDER_NOT_FOUND
    );
    assert_eq!(
        use_case
            .find_page(0, 20)
            .await
            .expect("empty page is valid"),
        (Vec::new(), 0)
    );
}
