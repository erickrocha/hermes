use business::domain::business_error::BusinessError;
use business::domain::business_plan::BusinessPlan;
use business::gateway::business_plan_gateway::BusinessPlanStore;
use business::use_cases::business_plan_use_case::BusinessPlanUseCase;
use chrono::NaiveDate;
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryBusinessPlanStore {
    plans: Mutex<Vec<BusinessPlan>>,
}

#[async_trait]
impl BusinessPlanStore for MemoryBusinessPlanStore {
    async fn save(&self, mut plan: BusinessPlan) -> Result<BusinessPlan, BusinessError> {
        let mut plans = self.plans.lock().map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        match plan.id {
            Some(id) => {
                if let Some(existing) = plans.iter_mut().find(|existing| existing.id == Some(id)) {
                    *existing = plan.clone();
                } else {
                    plans.push(plan.clone());
                }
            }
            None => {
                let id = plans.len() as i64 + 1;
                plan.id = Some(id);
                plan.uuid = Some(format!("business-plan-{id}"));
                plans.push(plan.clone());
            }
        }
        Ok(plan)
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<BusinessPlan>, BusinessError> {
        let plans = self.plans.lock().map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(plans.iter().find(|plan| plan.id == Some(id)).cloned())
    }

    async fn get_by_uuid(&self, uuid: &str) -> Result<Option<BusinessPlan>, BusinessError> {
        let plans = self.plans.lock().map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(plans.iter().find(|plan| plan.uuid.as_deref() == Some(uuid)).cloned())
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<BusinessPlan>, u64), BusinessError> {
        let plans = self.plans.lock().map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let filtered: Vec<_> = plans
            .iter()
            .filter(|plan| search.is_none_or(|query| plan.name.contains(query)))
            .cloned()
            .collect();
        let total = filtered.len() as u64;
        let start = (page * page_size) as usize;
        Ok((filtered.into_iter().skip(start).take(page_size as usize).collect(), total))
    }

    async fn list_all(&self) -> Result<Vec<BusinessPlan>, BusinessError> {
        let plans = self.plans.lock().map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(plans.clone())
    }

    async fn delete(&self, id: i64) -> Result<bool, BusinessError> {
        let mut plans = self.plans.lock().map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let before = plans.len();
        plans.retain(|plan| plan.id != Some(id));
        Ok(plans.len() != before)
    }
}

fn valid_plan() -> BusinessPlan {
    BusinessPlan {
        id: None,
        uuid: None,
        name: " Professional ".into(),
        price_in_cents: 10_000,
        available_users: 10,
        period_days: 30,
        payment_date: NaiveDate::from_ymd_opt(2026, 9, 30).unwrap_or_default(),
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn business_plan_use_case_runs_against_a_store_port_without_sea_orm() {
    let use_case = BusinessPlanUseCase::new(MemoryBusinessPlanStore::default());

    let created = use_case.create(valid_plan()).await.expect("valid plan is stored");
    let loaded = use_case.find_by_id(created.id.unwrap_or_default()).await.expect("plan is found");

    assert_eq!(created.name, "Professional");
    assert_eq!(loaded.uuid, created.uuid);
}