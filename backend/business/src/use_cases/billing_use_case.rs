use crate::domain::billing::{
    BillingAmount, BillingCharge, BillingChargeStatus, TenantBilling, calculate_billing_amount,
};
use crate::domain::business_error::BusinessError;
use crate::domain::business_plan::BusinessPlan;
use crate::domain::tenant::Tenant;
use crate::gateway::billing_gateway::BillingStore;
use crate::gateway::business_plan_gateway::BusinessPlanStore;
use crate::gateway::tenant_gateway::TenantStore;
use chrono::{Duration, NaiveDateTime};

pub struct BillingUseCase<B: BillingStore, T: TenantStore, P: BusinessPlanStore> {
    billing: B,
    tenants: T,
    plans: P,
}

impl<B: BillingStore, T: TenantStore, P: BusinessPlanStore> BillingUseCase<B, T, P> {
    pub fn new(billing: B, tenants: T, plans: P) -> Self {
        Self { billing, tenants, plans }
    }

    /// Initial assignment starts the first cycle. A later plan change is held
    /// as pending until the current cycle is closed.
    pub async fn assign_plan(
        &self,
        tenant_id: i64,
        business_plan_id: i64,
        now: NaiveDateTime,
    ) -> Result<BusinessPlan, BusinessError> {
        let plan = self.plans.get_by_id(business_plan_id).await?
            .ok_or_else(|| BusinessError::not_found("Business plan not found".to_string()))?;
        let mut tenant = self.tenants.get_by_id(tenant_id).await?
            .ok_or_else(|| BusinessError::not_found("Tenant not found".to_string()))?;
        let mut settings = self.settings_or_new(tenant_id).await?;

        match tenant.business_plan_id {
            None => {
                tenant.business_plan_id = Some(business_plan_id);
                self.tenants.save(tenant).await?;
                settings.cycle_started_at.get_or_insert(now);
                settings.pending_business_plan_id = None;
                self.billing.save_settings(settings).await?;
            }
            Some(current) if current == business_plan_id => {
                settings.pending_business_plan_id = None;
                self.billing.save_settings(settings).await?;
            }
            Some(_) => {
                settings.pending_business_plan_id = Some(business_plan_id);
                self.billing.save_settings(settings).await?;
            }
        }
        Ok(plan)
    }

    pub async fn get_settings(&self, tenant_id: i64) -> Result<Option<TenantBilling>, BusinessError> {
        self.billing.get_settings(tenant_id).await
    }

    /// There is deliberately no default: the first charge cannot close until
    /// the SysAdmin has explicitly configured a grace period for this tenant.
    pub async fn configure_grace_days(
        &self,
        tenant_id: i64,
        grace_days: i32,
        now: NaiveDateTime,
    ) -> Result<TenantBilling, BusinessError> {
        if grace_days < 0 {
            return Err(BusinessError::new("Grace days cannot be negative".to_string()));
        }
        let tenant = self.tenants.get_by_id(tenant_id).await?
            .ok_or_else(|| BusinessError::not_found("Tenant not found".to_string()))?;
        if tenant.business_plan_id.is_none() {
            return Err(BusinessError::new("A plan must be assigned before billing settings".to_string()));
        }
        let mut settings = self.settings_or_new(tenant_id).await?;
        settings.grace_days = Some(grace_days);
        settings.cycle_started_at.get_or_insert(now);
        self.billing.save_settings(settings).await
    }

    pub async fn set_suspension(
        &self,
        tenant_id: i64,
        suspended: bool,
        actor: &str,
        now: NaiveDateTime,
    ) -> Result<TenantBilling, BusinessError> {
        let _tenant = self.tenants.get_by_id(tenant_id).await?
            .ok_or_else(|| BusinessError::not_found("Tenant not found".to_string()))?;
        let mut settings = self.settings_or_new(tenant_id).await?;
        if suspended {
            if settings.suspended_at.is_none() {
                settings.suspended_at = Some(now);
                settings.suspended_by = Some(actor.to_string());
            }
        } else {
            settings.suspended_at = None;
            settings.suspended_by = None;
        }
        self.billing.save_settings(settings).await
    }

    pub async fn is_suspended(&self, tenant_id: i64) -> Result<bool, BusinessError> {
        Ok(self.billing.get_settings(tenant_id).await?
            .and_then(|settings| settings.suspended_at)
            .is_some())
    }

    pub async fn list_charges(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<BillingCharge>, u64), BusinessError> {
        self.billing.list_charges(page, page_size).await
    }

    pub async fn confirm_payment(
        &self,
        uuid: &str,
        actor: &str,
        now: NaiveDateTime,
    ) -> Result<BillingCharge, BusinessError> {
        let mut charge = self.billing.find_charge_by_uuid(uuid).await?
            .ok_or_else(|| BusinessError::not_found("Billing charge not found".to_string()))?;
        if charge.status == BillingChargeStatus::Pending {
            charge.status = BillingChargeStatus::Paid;
            charge.paid_at = Some(now);
            charge.paid_by = Some(actor.to_string());
            self.billing.save_charge(charge.clone()).await?;
        }
        Ok(charge)
    }

    /// One cycle per pass; the next scheduled pass catches up if the service
    /// was unavailable. The unique tenant/period key makes retries idempotent.
    pub async fn close_due_cycle(
        &self,
        tenant_id: i64,
        now: NaiveDateTime,
    ) -> Result<Option<BillingCharge>, BusinessError> {
        let Some(mut settings) = self.billing.get_settings(tenant_id).await? else {
            return Ok(None);
        };
        let Some(grace_days) = settings.grace_days else { return Ok(None) };
        let Some(period_start) = settings.cycle_started_at else {
            return Ok(None);
        };
        let tenant = self.tenants.get_by_id(tenant_id).await?
            .ok_or_else(|| BusinessError::not_found("Tenant not found".to_string()))?;
        let current_plan_id = tenant.business_plan_id
            .ok_or_else(|| BusinessError::new("Tenant has no billing plan".to_string()))?;
        let current_plan = self.plans.get_by_id(current_plan_id).await?
            .ok_or_else(|| BusinessError::not_found("Business plan not found".to_string()))?;
        let calculated_end = period_start + Duration::days(i64::from(current_plan.period_days));
        if calculated_end > now {
            return Ok(None);
        }

        // On retries, keep the original snapshot/end even if the following
        // plan assignment was applied before the settings row advanced.
        let existing = self.billing.find_charge_for_period(tenant_id, period_start).await?;
        let charge = if let Some(existing) = existing {
            existing
        } else {
            let active_users = self.billing.count_active_human_users(tenant_id).await?;
            let amount = calculate_billing_amount(
                current_plan.price_in_cents,
                current_plan.price_per_extra_user_in_cents,
                current_plan.available_users,
                active_users,
            )?;
            let new_charge = charge_for_cycle(&tenant, &current_plan, tenant_id, period_start, calculated_end, grace_days, amount);
            match self.billing.save_charge(new_charge).await {
                Ok(charge) => charge,
                Err(error) => match self.billing.find_charge_for_period(tenant_id, period_start).await? {
                    Some(charge) => charge,
                    None => return Err(error),
                },
            }
        };

        if let Some(next_plan_id) = settings.pending_business_plan_id {
            self.plans.get_by_id(next_plan_id).await?
                .ok_or_else(|| BusinessError::not_found("Pending business plan not found".to_string()))?;
            let mut tenant = tenant;
            tenant.business_plan_id = Some(next_plan_id);
            self.tenants.save(tenant).await?;
            settings.pending_business_plan_id = None;
        }
        settings.cycle_started_at = Some(charge.period_end);
        self.billing.save_settings(settings).await?;
        Ok(Some(charge))
    }

    async fn settings_or_new(&self, tenant_id: i64) -> Result<TenantBilling, BusinessError> {
        Ok(self.billing.get_settings(tenant_id).await?.unwrap_or(TenantBilling {
            id: None,
            uuid: None,
            tenant_id: Some(tenant_id),
            grace_days: None,
            cycle_started_at: None,
            pending_business_plan_id: None,
            suspended_at: None,
            suspended_by: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }))
    }
}

fn charge_for_cycle(
    tenant: &Tenant,
    plan: &BusinessPlan,
    tenant_id: i64,
    period_start: NaiveDateTime,
    period_end: NaiveDateTime,
    grace_days: i32,
    amount: BillingAmount,
) -> BillingCharge {
    BillingCharge {
        id: None,
        uuid: None,
        tenant_id: Some(tenant_id),
        tenant_uuid: tenant.uuid.clone().unwrap_or_default(),
        period_start,
        period_end,
        tenant_name: tenant.company_name.clone().unwrap_or_else(|| tenant.business_name.clone()),
        plan_name: plan.name.clone(),
        currency: "BRL".to_string(),
        price_in_cents: plan.price_in_cents,
        price_per_extra_user_in_cents: plan.price_per_extra_user_in_cents,
        included_users: plan.available_users,
        active_users: amount.active_users,
        extra_users: amount.extra_users,
        grace_days,
        total_in_cents: amount.total_in_cents,
        status: BillingChargeStatus::Pending,
        paid_at: None,
        paid_by: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}
