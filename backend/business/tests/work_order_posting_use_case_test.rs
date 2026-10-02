use business::domain::business_error::BusinessError;
use business::domain::part::Part;
use business::domain::stock_movement::StockMovement;
use business::domain::work_order::WorkOrder;
use business::domain::work_order_item::WorkOrderItem;
use business::domain::work_order_posting::WorkOrderPosting;
use business::gateway::work_order_posting_gateway::WorkOrderPostingStore;
use business::use_cases::stock_movement_use_case::StockBalanceReader;
use business::use_cases::work_order_posting_use_case::{
    QUANTITY_MUST_BE_POSITIVE, WorkOrderPostingUseCase,
};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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
