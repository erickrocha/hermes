use crate::domain::business_error::BusinessError;
use crate::domain::business_plan::{BusinessPlan, BusinessPlanEntityMapper};
use crate::gateway::business_plan_gateway::BusinessPlanGateway;

pub struct BusinessPlanUseCase {
    gateway: BusinessPlanGateway,
}

impl BusinessPlanUseCase {
    pub fn new(gateway: BusinessPlanGateway) -> Self {
        Self { gateway }
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
        if plan.daily_ai_quota <= 0 {
            return Err(BusinessError::new(
                "Daily AI quota must be greater than zero".to_string(),
            ));
        }
        if plan.price_in_cents < 0 {
            return Err(BusinessError::new(
                "Business plan prices cannot be negative".to_string(),
            ));
        }

        if !plan.tiers.is_empty() {
            let last_idx = plan.tiers.len() - 1;
            // A faixa sem teto (`up_to_users == 0`) tem de ser a última.
            if plan.tiers[last_idx].up_to_users != 0 {
                return Err(BusinessError::new(
                    "The last tier must have no user ceiling (up_to_users = 0)".to_string(),
                ));
            }

            let mut prev_threshold = plan.available_users;
            for (idx, tier) in plan.tiers.iter().enumerate() {
                if tier.price_per_user_in_cents < 0 {
                    return Err(BusinessError::new(
                        "Tier price per user cannot be negative".to_string(),
                    ));
                }

                if idx < last_idx {
                    if tier.up_to_users == 0 {
                        return Err(BusinessError::new(
                            "Only the last tier can have up_to_users = 0".to_string(),
                        ));
                    }
                    if tier.up_to_users <= prev_threshold {
                        if idx == 0 {
                            return Err(BusinessError::new(
                                "The first tier limit must be greater than available users"
                                    .to_string(),
                            ));
                        } else {
                            return Err(BusinessError::new(
                                "Tier limits must be strictly increasing without duplicates"
                                    .to_string(),
                            ));
                        }
                    }
                    prev_threshold = tier.up_to_users;
                }
            }
        }

        Ok(())
    }

    pub async fn create(&self, mut plan: BusinessPlan) -> Result<BusinessPlan, BusinessError> {
        Self::validate(&mut plan)?;
        plan.id = None;
        plan.uuid = None;
        let (saved, tiers) = self.gateway.save(plan).await.map_err(db_error)?;
        Ok(BusinessPlanEntityMapper::from_model_with_tiers(saved, tiers))
    }

    pub async fn find_by_id(&self, id: i64) -> Result<BusinessPlan, BusinessError> {
        self.gateway
            .find_by_id(id)
            .await
            .map_err(db_error)?
            .map(|(model, tiers)| BusinessPlanEntityMapper::from_model_with_tiers(model, tiers))
            .ok_or_else(|| BusinessError::new("Business plan not found".to_string()))
    }

    pub async fn find_by_uuid(&self, uuid: &str) -> Result<BusinessPlan, BusinessError> {
        self.gateway
            .find_by_uuid(uuid)
            .await
            .map_err(db_error)?
            .map(|(model, tiers)| BusinessPlanEntityMapper::from_model_with_tiers(model, tiers))
            .ok_or_else(|| BusinessError::new("Business plan not found".to_string()))
    }

    pub async fn find_all(&self) -> Result<Vec<BusinessPlan>, BusinessError> {
        self.gateway
            .find_all()
            .await
            .map(|list| {
                list.into_iter()
                    .map(|(model, tiers)| {
                        BusinessPlanEntityMapper::from_model_with_tiers(model, tiers)
                    })
                    .collect()
            })
            .map_err(db_error)
    }

    pub async fn update(&self, id: i64, mut plan: BusinessPlan) -> Result<BusinessPlan, BusinessError> {
        Self::validate(&mut plan)?;
        let existing = self.find_by_id(id).await?;
        plan.id = Some(id);
        plan.uuid = existing.uuid;
        plan.created_at = existing.created_at;
        plan.created_by = existing.created_by;
        let (saved, tiers) = self.gateway.save(plan).await.map_err(db_error)?;
        Ok(BusinessPlanEntityMapper::from_model_with_tiers(saved, tiers))
    }

    pub async fn delete(&self, id: i64) -> Result<(), BusinessError> {
        if !self.gateway.delete(id).await.map_err(db_error)? {
            return Err(BusinessError::new("Business plan not found".to_string()));
        }
        Ok(())
    }
}

fn db_error(error: sea_orm::DbErr) -> BusinessError {
    BusinessError::new(format!("Business plan database error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::BusinessPlanUseCase;
    use crate::domain::business_plan::BusinessPlan;
    use crate::domain::business_plan_tier::BusinessPlanTier;
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
            daily_ai_quota: 4,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
            tiers: Vec::new(),
        }
    }

    /// `up_to_users == 0` marca a faixa sem teto (a última).
    fn tier(up_to_users: i32, price_per_user_in_cents: i64) -> BusinessPlanTier {
        BusinessPlanTier {
            id: None,
            uuid: None,
            business_plan_id: 0,
            up_to_users,
            price_per_user_in_cents,
        }
    }

    #[test]
    fn validates_and_normalizes_a_valid_plan() {
        let mut plan = valid_plan();
        BusinessPlanUseCase::validate(&mut plan).unwrap();
        assert_eq!(plan.name, "Professional");
    }

    #[test]
    fn rejects_empty_name() {
        let mut plan = valid_plan();
        plan.name = "  ".to_string();
        assert!(BusinessPlanUseCase::validate(&mut plan).is_err());
    }

    #[test]
    fn rejects_non_positive_limits() {
        let mut plan = valid_plan();
        plan.available_users = 0;
        assert!(BusinessPlanUseCase::validate(&mut plan).is_err());

        let mut plan = valid_plan();
        plan.period_days = 0;
        assert!(BusinessPlanUseCase::validate(&mut plan).is_err());

        let mut plan = valid_plan();
        plan.daily_ai_quota = 0;
        assert!(BusinessPlanUseCase::validate(&mut plan).is_err());
    }

    #[test]
    fn allows_free_plans_and_rejects_negative_prices() {
        let mut free = valid_plan();
        free.price_in_cents = 0;
        BusinessPlanUseCase::validate(&mut free).unwrap();

        let mut invalid = valid_plan();
        invalid.price_in_cents = -1;
        assert!(BusinessPlanUseCase::validate(&mut invalid).is_err());
    }

    #[test]
    fn validates_valid_tiers() {
        let mut plan = valid_plan();
        plan.available_users = 15;
        plan.tiers = vec![tier(50, 900), tier(150, 700), tier(0, 500)];
        assert!(BusinessPlanUseCase::validate(&mut plan).is_ok());
    }

    #[test]
    fn rejects_tier_without_open_ended_at_end() {
        let mut plan = valid_plan();
        plan.available_users = 15;
        plan.tiers = vec![tier(50, 900)];
        assert!(BusinessPlanUseCase::validate(&mut plan).is_err());
    }

    #[test]
    fn rejects_tier_with_first_limit_not_above_available_users() {
        let mut plan = valid_plan();
        plan.available_users = 15;
        plan.tiers = vec![tier(15, 900), tier(0, 500)];
        assert!(BusinessPlanUseCase::validate(&mut plan).is_err());
    }

    #[test]
    fn rejects_unordered_or_duplicated_tiers() {
        let mut plan = valid_plan();
        plan.available_users = 15;
        plan.tiers = vec![tier(100, 900), tier(50, 700), tier(0, 500)];
        assert!(BusinessPlanUseCase::validate(&mut plan).is_err());
    }
}
