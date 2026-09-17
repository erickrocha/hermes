use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::tenant::{Tenant, TenantEntityMapper};
use crate::gateway::tenant_gateway::TenantGateway;

fn valid_country_code(value: &Option<String>) -> bool {
    value.as_ref().is_none_or(|code| code.len() == 2 && code.chars().all(|c| c.is_ascii_alphabetic()))
}

pub struct TenantUseCase {
    gateway: TenantGateway,
}

impl TenantUseCase {
    pub fn new(gateway: TenantGateway) -> Self {
        Self { gateway }
    }

    pub async fn create(&self, tenant: Tenant) -> Result<Tenant, BusinessError> {
        log::info!("[TenantUseCase::create] Executing create tenant for business name: {:?}", tenant.business_name);

        if tenant.business_name.is_empty() {
            let msg = "Tenant business name is required".to_string();
            log::error!("[TenantUseCase::create] {}", msg);
            return Err(BusinessError::new(msg));
        }
        if !valid_country_code(&tenant.country_code) {
            return Err(BusinessError::new("Country code must contain two letters".to_string()));
        }

        // A tenant's plan is set only through `set_plan` (HRMS-224, PD-021),
        // never at creation, regardless of what the caller sent.
        let tenant = Tenant { business_plan_id: None, ..tenant };

        let entity = self
            .gateway
            .persist(tenant)
            .await
            .map_err(|e| {
                let msg = format!("Failed to persist tenant: {}", e);
                log::error!("[TenantUseCase::create] {}", msg);
                BusinessError::new(msg)
            })?;

        Ok(TenantEntityMapper::from_active_model(entity))
    }

    pub async fn find_by_id(&self, id: i64) -> Result<Tenant, BusinessError> {
        log::info!("[TenantUseCase::find_by_id] Executing for id: {}", id);

        let entity = self
            .gateway
            .find_by_id(id)
            .await
            .map_err(|e| {
                let msg = format!("Database error: {}", e);
                log::error!("[TenantUseCase::find_by_id] {}", msg);
                BusinessError::new(msg)
            })?;

        match entity {
            Some(value) => Ok(TenantEntityMapper::from_model(value)),
            None => {
                let msg = format!("Tenant not found with id: {}", id);
                log::error!("[TenantUseCase::find_by_id] {}", msg);
                Err(BusinessError::new("Tenant not found".to_string()))
            }
        }
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<Tenant, BusinessError> {
        log::info!("[TenantUseCase::find_by_uuid] Executing for uuid: {}", uuid);

        let entity = self
            .gateway
            .find_by_uuid(uuid.clone())
            .await
            .map_err(|e| {
                let msg = format!("Database error: {}", e);
                log::error!("[TenantUseCase::find_by_uuid] {}", msg);
                BusinessError::new(msg)
            })?;

        match entity {
            Some(value) => Ok(TenantEntityMapper::from_model(value)),
            None => {
                let msg = format!("Tenant not found with uuid: {}", uuid);
                log::error!("[TenantUseCase::find_by_uuid] {}", msg);
                Err(BusinessError::new("Tenant not found".to_string()))
            }
        }
    }

    pub async fn find_all(&self) -> Result<Vec<Tenant>, BusinessError> {
        log::info!("[TenantUseCase::find_all] Executing find_all tenants");

        let entities = self
            .gateway
            .find_all()
            .await
            .map_err(|e| {
                let msg = format!("Database error: {}", e);
                log::error!("[TenantUseCase::find_all] {}", msg);
                BusinessError::new(msg)
            })?;

        Ok(TenantEntityMapper::from_models(entities))
    }

    pub async fn update(&self, id: i64, tenant: Tenant) -> Result<Tenant, BusinessError> {
        log::info!("[TenantUseCase::update] Executing update for id {}: {:?}", id, tenant.business_name);

        let existing = match self.find_by_id(id).await {
            Ok(t) => t,
            Err(e) => {
                log::error!("[TenantUseCase::update] Tenant to update not found with id {}: {}", id, e);
                return Err(e);
            }
        };

        if tenant.company_name.as_ref().is_none_or(|n| n.trim().is_empty()) {
            let msg = "Tenant name is required".to_string();
            log::error!("[TenantUseCase::update] {}", msg);
            return Err(BusinessError::new(msg));
        }
        if !valid_country_code(&tenant.country_code) {
            return Err(BusinessError::new("Country code must contain two letters".to_string()));
        }
        if tenant.payment_grace_days.is_some_and(|days| days < 0) {
            return Err(BusinessError::new("Payment grace days cannot be negative".to_string()));
        }

        let updated_tenant = Tenant {
            id: Some(id),
            uuid: existing.uuid,
            company_name: tenant.company_name,
            business_name:  tenant.business_name,
            tax_id: tenant.tax_id,
            email: tenant.email,
            phone: tenant.phone,
            website: tenant.website,
            address_line1: tenant.address_line1,
            address_line2: tenant.address_line2,
            locality: tenant.locality,
            administrative_area: tenant.administrative_area,
            postal_code: tenant.postal_code,
            country_code: tenant.country_code,
            // Omitting the field keeps the clinic's current grace rather than
            // silently resetting it to the column default.
            payment_grace_days: tenant.payment_grace_days.or(existing.payment_grace_days),
            // A tenant's plan is set only through `set_plan` (HRMS-224,
            // PD-021) — the general update path always keeps it as-is,
            // regardless of what the caller sent.
            business_plan_id: existing.business_plan_id,
            created_at: existing.created_at,
            created_by: existing.created_by,
            updated_at: None,
            updated_by: tenant.updated_by,
        };

        let entity = self
            .gateway
            .persist(updated_tenant)
            .await
            .map_err(|e| {
                let msg = format!("Failed to update tenant: {}", e);
                log::error!("[TenantUseCase::update] {}", msg);
                BusinessError::new(msg)
            })?;

        Ok(TenantEntityMapper::from_active_model(entity))
    }

    pub async fn persist(&self, tenant: Tenant) -> Option<Tenant> {
        log::info!("[TenantUseCase::persist] Executing persist tenant: {:?}", tenant.business_name);
        self.create(tenant).await.ok()
    }

    /// Sets the tenant's single current plan (HRMS-222, PD-021). The caller
    /// (the `/tenant/{id}/plan` endpoint) is responsible for restricting this
    /// to an unbound platform administrator and for validating that
    /// `business_plan_id` refers to an existing plan (HRMS-224).
    pub async fn set_plan(&self, tenant_id: i64, business_plan_id: i64) -> Result<Tenant, BusinessError> {
        log::info!("[TenantUseCase::set_plan] Setting plan {} for tenant {}", business_plan_id, tenant_id);

        let existing = self.find_by_id(tenant_id).await?;
        let updated = Tenant {
            business_plan_id: Some(business_plan_id),
            ..existing
        };

        let entity = self
            .gateway
            .persist(updated)
            .await
            .map_err(|e| {
                let msg = format!("Failed to set tenant plan: {}", e);
                log::error!("[TenantUseCase::set_plan] {}", msg);
                BusinessError::new(msg)
            })?;

        Ok(TenantEntityMapper::from_active_model(entity))
    }
}
