use crate::domain::business_error::BusinessError;
use crate::domain::business_plan::BusinessPlan;
use crate::gateway::business_plan_gateway::BusinessPlanStore;

pub struct BusinessPlanUseCase<G: BusinessPlanStore> {
    gateway: G,
}

impl<G: BusinessPlanStore> BusinessPlanUseCase<G> {
    pub fn new(gateway: G) -> Self {
        Self { gateway }
    }

    pub async fn create(&self, mut plan: BusinessPlan) -> Result<BusinessPlan, BusinessError> {
        validate(&mut plan)?;
        plan.id = None;
        plan.uuid = None;
        self.gateway.save(plan).await
    }

    pub async fn find_by_id(&self, id: i64) -> Result<BusinessPlan, BusinessError> {
        self.gateway
            .get_by_id(id)
            .await
            ?
            .ok_or_else(|| BusinessError::new("Business plan not found".to_string()))
    }

    pub async fn find_by_uuid(&self, uuid: &str) -> Result<BusinessPlan, BusinessError> {
        self.gateway
            .get_by_uuid(uuid)
            .await
            ?
            .ok_or_else(|| BusinessError::new("Business plan not found".to_string()))
    }

    /// PD-028.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<BusinessPlan>, u64), BusinessError> {
        self
            .gateway
            .list_page(page, page_size, search)
            .await
    }

    pub async fn find_all(&self) -> Result<Vec<BusinessPlan>, BusinessError> {
        self.gateway.list_all().await
    }

    pub async fn update(
        &self,
        id: i64,
        mut plan: BusinessPlan,
    ) -> Result<BusinessPlan, BusinessError> {
        validate(&mut plan)?;
        let existing = self.find_by_id(id).await?;
        plan.id = Some(id);
        plan.uuid = existing.uuid;
        plan.created_at = existing.created_at;
        plan.created_by = existing.created_by;
        self.gateway.save(plan).await
    }

    pub async fn delete(&self, id: i64) -> Result<(), BusinessError> {
        if !self.gateway.delete(id).await? {
            return Err(BusinessError::new("Business plan not found".to_string()));
        }
        Ok(())
    }
}

fn validate(plan: &mut BusinessPlan) -> Result<(), BusinessError> {
    plan.name = plan.name.trim().to_string();
    if plan.name.is_empty() {
        return Err(BusinessError::new(
            "Business plan name is required".to_string(),
        ));
    }
    if plan.available_users <= 0 {
        return Err(BusinessError::new(
            "Available users must be greater than zero".to_string(),
        ));
    }
    if plan.period_days <= 0 {
        return Err(BusinessError::new(
            "Period days must be greater than zero".to_string(),
        ));
    }
    if plan.price_in_cents < 0 {
        return Err(BusinessError::new(
            "Business plan prices cannot be negative".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate;
    use crate::domain::business_plan::BusinessPlan;
    use chrono::NaiveDate;

    fn valid_plan() -> BusinessPlan {
        BusinessPlan {
            id: None,
            uuid: None,
            name: " Professional ".to_string(),
            price_in_cents: 10_000,
            available_users: 10,
            period_days: 30,
            payment_date: NaiveDate::from_ymd_opt(2026, 9, 30).unwrap(),
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }
    }

    #[test]
    fn validates_and_normalizes_a_valid_plan() {
        let mut plan = valid_plan();
        validate(&mut plan).unwrap();
        assert_eq!(plan.name, "Professional");
    }

    #[test]
    fn rejects_empty_name() {
        let mut plan = valid_plan();
        plan.name = "  ".to_string();
        assert!(validate(&mut plan).is_err());
    }

    #[test]
    fn rejects_non_positive_limits() {
        let mut plan = valid_plan();
        plan.available_users = 0;
        assert!(validate(&mut plan).is_err());

        let mut plan = valid_plan();
        plan.period_days = 0;
            assert!(validate(&mut plan).is_err());
    }

    #[test]
    fn allows_free_plans_and_rejects_negative_prices() {
        let mut free = valid_plan();
        free.price_in_cents = 0;
        validate(&mut free).unwrap();

        let mut invalid = valid_plan();
        invalid.price_in_cents = -1;
        assert!(validate(&mut invalid).is_err());
    }
}
