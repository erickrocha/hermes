use business::domain::business_error::BusinessError;
use business::domain::enums::PreventiveControlType;
use business::domain::preventive_plan::PreventivePlan;
use business::domain::preventive_plan_extension::PreventivePlanExtension;
use business::domain::tenant_rule_setting::RuleSettings;
use business::domain::vehicle::Vehicle;
use business::domain::work_order::WorkOrder;
use business::domain::work_order_item::WorkOrderItem;
use business::gateway::preventive_plan_gateway::PreventivePlanStore;
use business::use_cases::preventive_plan_use_case::{PLAN_NAME_REQUIRED, PreventivePlanUseCase};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct MemoryPreventivePlanStore {
    calls: Arc<AtomicUsize>,
}

impl MemoryPreventivePlanStore {
    fn touch(&self) {
        self.calls.fetch_add(1, Ordering::Relaxed);
    }
}

#[async_trait]
impl PreventivePlanStore for MemoryPreventivePlanStore {
    async fn save_plan(&self, plan: PreventivePlan) -> Result<PreventivePlan, BusinessError> {
        self.touch();
        Ok(plan)
    }

    async fn find_plan(&self, _uuid: String) -> Result<Option<PreventivePlan>, BusinessError> {
        self.touch();
        Ok(None)
    }

    async fn find_plan_by_vehicle_and_name(
        &self,
        _vehicle_id: i64,
        _name: &str,
    ) -> Result<Option<PreventivePlan>, BusinessError> {
        self.touch();
        Ok(None)
    }

    async fn list_plans(
        &self,
        _page: u64,
        _page_size: u64,
    ) -> Result<(Vec<PreventivePlan>, u64), BusinessError> {
        self.touch();
        Ok((Vec::new(), 0))
    }

    async fn find_vehicle(&self, _id: i64) -> Result<Option<Vehicle>, BusinessError> {
        self.touch();
        Ok(None)
    }

    async fn find_work_order(&self, _id: i64) -> Result<Option<WorkOrder>, BusinessError> {
        self.touch();
        Ok(None)
    }

    async fn active_work_orders(&self, _plan_id: i64) -> Result<Vec<WorkOrder>, BusinessError> {
        self.touch();
        Ok(Vec::new())
    }

    async fn save_work_order(&self, order: WorkOrder) -> Result<WorkOrder, BusinessError> {
        self.touch();
        Ok(order)
    }

    async fn find_work_order_item(
        &self,
        _uuid: String,
    ) -> Result<Option<WorkOrderItem>, BusinessError> {
        self.touch();
        Ok(None)
    }

    async fn save_work_order_item(
        &self,
        item: WorkOrderItem,
    ) -> Result<WorkOrderItem, BusinessError> {
        self.touch();
        Ok(item)
    }

    async fn save_extension(
        &self,
        extension: PreventivePlanExtension,
    ) -> Result<PreventivePlanExtension, BusinessError> {
        self.touch();
        Ok(extension)
    }

    async fn rules(&self, _tenant_id: Option<i64>) -> Result<RuleSettings, BusinessError> {
        self.touch();
        Ok(RuleSettings::default())
    }
}

fn plan(name: &str) -> PreventivePlan {
    PreventivePlan {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        vehicle_id: 10,
        plan_name: name.into(),
        control_type: PreventiveControlType::Kilometers,
        interval_km: Some(10_000.0),
        interval_days: None,
        last_service_km: Some(50_000.0),
        last_service_date: None,
        extension_limit_km: None,
        last_work_order_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn a_blank_plan_name_is_rejected_before_any_store_call() {
    let calls = Arc::new(AtomicUsize::new(0));
    let use_case = PreventivePlanUseCase::new(MemoryPreventivePlanStore {
        calls: Arc::clone(&calls),
    });

    let error = use_case
        .create(plan("  "))
        .await
        .expect_err("plan name is required");

    assert_eq!(error.message, PLAN_NAME_REQUIRED);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}
