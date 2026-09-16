use crate::commons::gateway::Gateway;
use crate::commons::entity_mapper::EntityMapper;
use crate::domain::business_error::BusinessError;
use crate::domain::tenant_plan::TenantPlan;
use crate::gateway::tenant_plan_gateway::TenantPlanGateway;
use uuid::Uuid;

pub struct TenantPlanUseCase {
    gateway: TenantPlanGateway,
}

impl TenantPlanUseCase {
    pub fn new(gateway: TenantPlanGateway) -> Self {
        Self { gateway }
    }

    pub async fn create_or_update(&self, mut tenant_plan: TenantPlan) -> Result<TenantPlan, BusinessError> {
        if tenant_plan.uuid.is_none() {
            tenant_plan.uuid = Some(Uuid::new_v4().to_string());
        }

        if tenant_plan.active {
            // Deactivate all previously active plans for this tenant
            self.gateway
                .deactivate_all_active_plans_for_tenant(tenant_plan.tenant_id)
                .await
                .map_err(|e| BusinessError::new(e.to_string()))?;
        }

        let saved = self
            .gateway
            .persist(tenant_plan)
            .await
            .map_err(|e| BusinessError::new(e.to_string()))?;

        // After save, we should return the saved TenantPlan.
        // It requires mapping from ActiveModel to TenantPlan, which we usually do via an entity find or mapper.
        // For simplicity, we can fetch it again using its UUID.
        let uuid_bytes = saved.uuid.unwrap();
        let uuid_str = crate::commons::functions::bytes_para_string(uuid_bytes);
        
        let found = self
            .gateway
            .find_by_uuid(uuid_str)
            .await
            .map_err(|e| BusinessError::new(e.to_string()))?
            .ok_or_else(|| BusinessError::new("TenantPlan not found after save".to_string()))?;

        Ok(crate::domain::tenant_plan::TenantPlanEntityMapper::from_model(found))
    }

    pub async fn find_active_by_tenant_id(&self, tenant_id: i64) -> Result<Option<TenantPlan>, BusinessError> {
        self.gateway
            .find_active_by_tenant_id(tenant_id)
            .await
            .map(|opt| opt.map(crate::domain::tenant_plan::TenantPlanEntityMapper::from_model))
            .map_err(|e| BusinessError::new(e.to_string()))
    }
}
