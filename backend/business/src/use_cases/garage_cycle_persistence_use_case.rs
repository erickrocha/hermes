use crate::domain::business_error::BusinessError;
use crate::domain::garage_cycle::GarageCycleState;
use crate::domain::tenant_rule_setting::RuleSettings;
use crate::gateway::garage_cycle_gateway::GarageCycleStore;
use crate::use_cases::garage_cycle::{CycleSampleResult, GarageCycleSample, observe};

pub struct GarageCyclePersistenceUseCase<G: GarageCycleStore> {
    store: G,
}

impl<G: GarageCycleStore> GarageCyclePersistenceUseCase<G> {
    pub fn new(store: G) -> Self {
        Self { store }
    }

    pub async fn state_for_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<GarageCycleState>, BusinessError> {
        self.store.find_by_vehicle(vehicle_id).await
    }

    pub async fn observe(
        &self,
        sample: GarageCycleSample,
        rules: &RuleSettings,
    ) -> Result<CycleSampleResult, BusinessError> {
        let state = self
            .state_for_vehicle(sample.vehicle_id)
            .await?
            .unwrap_or_else(|| GarageCycleState::new(sample.tenant_id, sample.vehicle_id));
        let mut updated = state.clone();
        let result = observe(&mut updated, sample, rules);
        if result == CycleSampleResult::Updated && updated != state {
            self.store.save(updated).await?;
        }
        Ok(result)
    }

    pub async fn last_completed_distance_km(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<f64>, BusinessError> {
        Ok(self
            .state_for_vehicle(vehicle_id)
            .await?
            .and_then(|state| state.last_completed_cycle_distance_meters)
            .map(|meters| meters / 1000.0))
    }
}
