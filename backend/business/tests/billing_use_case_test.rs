use business::domain::billing::{BillingCharge, BillingChargeStatus, TenantBilling};
use business::domain::business_error::BusinessError;
use business::domain::business_plan::BusinessPlan;
use business::domain::tenant::Tenant;
use business::gateway::billing_gateway::BillingStore;
use business::gateway::business_plan_gateway::BusinessPlanStore;
use business::gateway::tenant_gateway::TenantStore;
use business::sea_orm::prelude::async_trait::async_trait;
use business::use_cases::billing_use_case::BillingUseCase;
use chrono::{NaiveDate, NaiveDateTime};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct MemoryTenantStore {
    tenants: Arc<Mutex<Vec<Tenant>>>,
}

#[async_trait]
impl TenantStore for MemoryTenantStore {
    async fn save(&self, mut tenant: Tenant) -> Result<Tenant, BusinessError> {
        let mut tenants = self.tenants.lock().unwrap();
        if let Some(id) = tenant.id {
            if let Some(existing) = tenants.iter_mut().find(|existing| existing.id == Some(id)) {
                *existing = tenant.clone();
            } else {
                tenants.push(tenant.clone());
            }
        } else {
            tenant.id = Some(1);
            tenant.uuid = Some("tenant-uuid-1".to_string());
            tenants.push(tenant.clone());
        }
        Ok(tenant)
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<Tenant>, BusinessError> {
        Ok(self.tenants.lock().unwrap().iter().find(|tenant| tenant.id == Some(id)).cloned())
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<Tenant>, BusinessError> {
        Ok(self.tenants.lock().unwrap().iter().find(|tenant| tenant.uuid.as_deref() == Some(&uuid)).cloned())
    }

    async fn list_page(&self, page: u64, page_size: u64, _search: Option<&str>) -> Result<(Vec<Tenant>, u64), BusinessError> {
        let tenants = self.tenants.lock().unwrap();
        let total = tenants.len() as u64;
        let start = (page * page_size) as usize;
        Ok((tenants.iter().skip(start).take(page_size as usize).cloned().collect(), total))
    }

    async fn list_all(&self) -> Result<Vec<Tenant>, BusinessError> {
        Ok(self.tenants.lock().unwrap().clone())
    }
}

#[derive(Clone)]
struct MemoryPlanStore {
    plans: Arc<Vec<BusinessPlan>>,
}

#[async_trait]
impl BusinessPlanStore for MemoryPlanStore {
    async fn save(&self, plan: BusinessPlan) -> Result<BusinessPlan, BusinessError> { Ok(plan) }
    async fn get_by_id(&self, id: i64) -> Result<Option<BusinessPlan>, BusinessError> {
        Ok(self.plans.iter().find(|plan| plan.id == Some(id)).cloned())
    }
    async fn get_by_uuid(&self, uuid: &str) -> Result<Option<BusinessPlan>, BusinessError> {
        Ok(self.plans.iter().find(|plan| plan.uuid.as_deref() == Some(uuid)).cloned())
    }
    async fn list_page(&self, page: u64, page_size: u64, _search: Option<&str>) -> Result<(Vec<BusinessPlan>, u64), BusinessError> {
        let total = self.plans.len() as u64;
        let start = (page * page_size) as usize;
        Ok((self.plans.iter().skip(start).take(page_size as usize).cloned().collect(), total))
    }
    async fn list_all(&self) -> Result<Vec<BusinessPlan>, BusinessError> { Ok(self.plans.as_ref().clone()) }
    async fn delete(&self, _id: i64) -> Result<bool, BusinessError> { Ok(false) }
}

#[derive(Clone)]
struct MemoryBillingStore {
    settings: Arc<Mutex<Vec<TenantBilling>>>,
    charges: Arc<Mutex<Vec<BillingCharge>>>,
    active_users: i32,
}

#[async_trait]
impl BillingStore for MemoryBillingStore {
    async fn get_settings(&self, tenant_id: i64) -> Result<Option<TenantBilling>, BusinessError> {
        Ok(self.settings.lock().unwrap().iter().find(|settings| settings.tenant_id == Some(tenant_id)).cloned())
    }

    async fn save_settings(&self, mut settings: TenantBilling) -> Result<TenantBilling, BusinessError> {
        let mut stored = self.settings.lock().unwrap();
        if let Some(existing) = stored.iter_mut().find(|existing| existing.tenant_id == settings.tenant_id) {
            settings.id = existing.id;
            settings.uuid = existing.uuid.clone();
            *existing = settings.clone();
        } else {
            settings.id = Some(1);
            settings.uuid = Some("billing-uuid-1".to_string());
            stored.push(settings.clone());
        }
        Ok(settings)
    }

    async fn count_active_human_users(&self, _tenant_id: i64) -> Result<i32, BusinessError> {
        Ok(self.active_users)
    }

    async fn find_charge_by_uuid(&self, uuid: &str) -> Result<Option<BillingCharge>, BusinessError> {
        Ok(self.charges.lock().unwrap().iter().find(|charge| charge.uuid.as_deref() == Some(uuid)).cloned())
    }

    async fn find_charge_for_period(&self, tenant_id: i64, period_start: NaiveDateTime) -> Result<Option<BillingCharge>, BusinessError> {
        Ok(self.charges.lock().unwrap().iter().find(|charge| charge.tenant_id == Some(tenant_id) && charge.period_start == period_start).cloned())
    }

    async fn save_charge(&self, mut charge: BillingCharge) -> Result<BillingCharge, BusinessError> {
        let mut charges = self.charges.lock().unwrap();
        if let Some(id) = charge.id
            && let Some(existing) = charges.iter_mut().find(|existing| existing.id == Some(id))
        {
            *existing = charge.clone();
            return Ok(charge);
        }
        if let Some(existing) = charges.iter().find(|existing| existing.tenant_id == charge.tenant_id && existing.period_start == charge.period_start) {
            return Ok(existing.clone());
        }
        charge.id = Some(charges.len() as i64 + 1);
        charge.uuid = Some(format!("charge-{}", charges.len() + 1));
        charges.push(charge.clone());
        Ok(charge)
    }

    async fn list_charges(&self, page: u64, page_size: u64) -> Result<(Vec<BillingCharge>, u64), BusinessError> {
        let charges = self.charges.lock().unwrap();
        let total = charges.len() as u64;
        let start = (page * page_size) as usize;
        Ok((charges.iter().skip(start).take(page_size as usize).cloned().collect(), total))
    }
}

fn at(year: i32, month: u32, day: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(year, month, day).unwrap().and_hms_opt(0, 0, 0).unwrap()
}

fn plan(id: i64, name: &str, base: i64, included: i32, overage: i64, days: i32) -> BusinessPlan {
    BusinessPlan {
        id: Some(id), uuid: Some(format!("plan-{id}")), name: name.to_string(),
        price_in_cents: base, price_per_extra_user_in_cents: overage,
        available_users: included, period_days: days,
        created_at: None, created_by: None, updated_at: None, updated_by: None,
    }
}

fn tenant() -> Tenant {
    Tenant {
        id: Some(1), uuid: Some("tenant-uuid-1".to_string()), business_name: "Acme".to_string(),
        company_name: Some("Acme Transport".to_string()), tax_id: "123".to_string(), email: None,
        phone: None, website: None, address_line1: None, address_line2: None, locality: None,
        administrative_area: None, postal_code: None, country_code: Some("BR".to_string()),
        business_plan_id: None, created_at: None, updated_at: None, created_by: None, updated_by: None,
    }
}

fn setup(active_users: i32) -> (BillingUseCase<MemoryBillingStore, MemoryTenantStore, MemoryPlanStore>, MemoryBillingStore, MemoryTenantStore) {
    let tenants = MemoryTenantStore::default();
    tenants.tenants.lock().unwrap().push(tenant());
    let billing = MemoryBillingStore {
        settings: Arc::new(Mutex::new(Vec::new())),
        charges: Arc::new(Mutex::new(Vec::new())),
        active_users,
    };
    let plans = MemoryPlanStore { plans: Arc::new(vec![plan(1, "Pro", 10_000, 10, 500, 30), plan(2, "Pro+", 15_000, 15, 400, 30)]) };
    (BillingUseCase::new(billing.clone(), tenants.clone(), plans), billing, tenants)
}

#[tokio::test]
async fn assignment_starts_cycle_and_plan_change_waits_for_cycle_close() {
    let (use_case, billing, tenants) = setup(13);
    let start = at(2026, 1, 1);
    use_case.assign_plan(1, 1, start).await.unwrap();
    use_case.configure_grace_days(1, 7, start).await.unwrap();
    use_case.assign_plan(1, 2, at(2026, 1, 10)).await.unwrap();

    assert_eq!(tenants.get_by_id(1).await.unwrap().unwrap().business_plan_id, Some(1));
    assert_eq!(billing.get_settings(1).await.unwrap().unwrap().pending_business_plan_id, Some(2));
    assert!(use_case.close_due_cycle(1, at(2026, 1, 30)).await.unwrap().is_none());

    let charge = use_case.close_due_cycle(1, at(2026, 1, 31)).await.unwrap().unwrap();
    assert_eq!(charge.period_start, start);
    assert_eq!(charge.period_end, at(2026, 1, 31));
    assert_eq!(charge.active_users, 13);
    assert_eq!(charge.extra_users, 3);
    assert_eq!(charge.total_in_cents, 11_500);
    assert_eq!(tenants.get_by_id(1).await.unwrap().unwrap().business_plan_id, Some(2));
    assert_eq!(billing.get_settings(1).await.unwrap().unwrap().pending_business_plan_id, None);
}

#[tokio::test]
async fn cycle_close_requires_grace_configuration_and_retries_are_idempotent() {
    let (use_case, billing, _) = setup(10);
    let start = at(2026, 1, 1);
    use_case.assign_plan(1, 1, start).await.unwrap();
    assert!(use_case.close_due_cycle(1, at(2026, 1, 31)).await.unwrap().is_none());

    use_case.configure_grace_days(1, 0, start).await.unwrap();
    let first = use_case.close_due_cycle(1, at(2026, 1, 31)).await.unwrap().unwrap();
    let retry = use_case.close_due_cycle(1, at(2026, 1, 31)).await.unwrap();
    assert!(retry.is_none());
    assert_eq!(billing.charges.lock().unwrap().len(), 1);
    assert_eq!(first.status, BillingChargeStatus::Pending);
}

#[tokio::test]
async fn payment_confirmation_is_idempotent_and_suspension_is_manual() {
    let (use_case, _, _) = setup(10);
    let start = at(2026, 1, 1);
    use_case.assign_plan(1, 1, start).await.unwrap();
    use_case.configure_grace_days(1, 7, start).await.unwrap();
    let charge = use_case.close_due_cycle(1, at(2026, 1, 31)).await.unwrap().unwrap();
    let uuid = charge.uuid.unwrap();
    let paid = use_case.confirm_payment(&uuid, "finance@hermes.test", at(2026, 2, 1)).await.unwrap();
    let paid_again = use_case.confirm_payment(&uuid, "other@hermes.test", at(2026, 2, 2)).await.unwrap();
    assert_eq!(paid.status, BillingChargeStatus::Paid);
    assert_eq!(paid_again.paid_by.as_deref(), Some("finance@hermes.test"));

    assert!(!use_case.is_suspended(1).await.unwrap());
    use_case.set_suspension(1, true, "sysadmin@hermes.test", at(2026, 2, 2)).await.unwrap();
    assert!(use_case.is_suspended(1).await.unwrap());
    use_case.set_suspension(1, false, "sysadmin@hermes.test", at(2026, 2, 3)).await.unwrap();
    assert!(!use_case.is_suspended(1).await.unwrap());
}