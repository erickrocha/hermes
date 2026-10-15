use business::domain::business_error::BusinessError;
use business::domain::part::Part;
use business::domain::purchase_order::PurchaseOrder;
use business::domain::work_order::WorkOrder;
use business::domain::work_order_item::WorkOrderItem;
use business::gateway::purchase_order_gateway::PurchaseOrderStore;
use business::use_cases::purchase_order_use_case::{
    PurchaseOrderUseCase, PurchaseOrderWorkOrderService, QUANTITY_MUST_BE_POSITIVE,
};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct MemoryPurchaseOrderStore {
    reads: Arc<AtomicUsize>,
}

#[async_trait]
impl PurchaseOrderStore for MemoryPurchaseOrderStore {
    async fn find_part(&self, _part_id: i64) -> Result<Option<Part>, BusinessError> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        Ok(None)
    }

    async fn save(&self, order: PurchaseOrder) -> Result<PurchaseOrder, BusinessError> {
        Ok(order)
    }

    async fn find_by_id(&self, _id: i64) -> Result<Option<PurchaseOrder>, BusinessError> {
        Ok(None)
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

#[tokio::test]
async fn a_non_positive_quantity_is_rejected_before_any_store_or_work_order_call() {
    let reads = Arc::new(AtomicUsize::new(0));
    let store = MemoryPurchaseOrderStore {
        reads: Arc::clone(&reads),
    };
    let use_case = PurchaseOrderUseCase::new(store, NoWorkOrderOperations);

    let error = use_case
        .create(order(0.0))
        .await
        .expect_err("purchase quantity must be positive");

    assert_eq!(error.message, QUANTITY_MUST_BE_POSITIVE);
    assert_eq!(reads.load(Ordering::Relaxed), 0);
}
