use business::domain::business_error::BusinessError;
use business::domain::garage_cycle::GarageCycleState;
use business::domain::tenant_rule_setting::RuleSettings;
use business::gateway::garage_cycle_gateway::GarageCycleStore;
use business::use_cases::garage_cycle::{CycleSampleResult, GarageCycleSample};
use business::use_cases::garage_cycle_persistence_use_case::GarageCyclePersistenceUseCase;
use chrono::NaiveDate;
use sea_orm::prelude::async_trait::async_trait;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct MemoryCycleStore {
    state: Arc<Mutex<Option<GarageCycleState>>>,
    saves: Arc<Mutex<usize>>,
}

#[async_trait]
impl GarageCycleStore for MemoryCycleStore {
    async fn find_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<GarageCycleState>, BusinessError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| BusinessError::new("cycle store poisoned".into()))?
            .as_ref()
            .filter(|state| state.vehicle_id == vehicle_id)
            .cloned())
    }

    async fn save(&self, state: GarageCycleState) -> Result<GarageCycleState, BusinessError> {
        *self
            .state
            .lock()
            .map_err(|_| BusinessError::new("cycle store poisoned".into()))? = Some(state.clone());
        *self
            .saves
            .lock()
            .map_err(|_| BusinessError::new("cycle counter poisoned".into()))? += 1;
        Ok(state)
    }
}

fn sample(hour: u32, odometer_meters: Option<f64>) -> GarageCycleSample {
    GarageCycleSample {
        tenant_id: Some(42),
        vehicle_id: 7,
        reported_at: NaiveDate::from_ymd_opt(2026, 10, 21)
            .unwrap()
            .and_hms_opt(hour, 0, 0)
            .unwrap(),
        at_base: true,
        transition: None,
        odometer_meters,
        ignition_on: Some(true),
    }
}

#[tokio::test]
async fn persistence_creates_state_on_first_observation_and_skips_unchanged_samples() {
    let store = MemoryCycleStore::default();
    let use_case = GarageCyclePersistenceUseCase::new(store.clone());

    assert_eq!(
        use_case
            .observe(sample(8, Some(10_000.0)), &RuleSettings::default())
            .await
            .unwrap(),
        CycleSampleResult::Updated
    );
    assert_eq!(
        use_case
            .observe(sample(8, Some(11_000.0)), &RuleSettings::default())
            .await
            .unwrap(),
        CycleSampleResult::Ignored
    );
    assert_eq!(*store.saves.lock().unwrap(), 1);
    assert_eq!(
        use_case
            .state_for_vehicle(7)
            .await
            .unwrap()
            .unwrap()
            .last_observed_odometer_meters,
        Some(10_000.0)
    );
}

#[tokio::test]
async fn persistence_converts_the_last_completed_distance_to_kilometers() {
    let store = MemoryCycleStore::default();
    *store.state.lock().unwrap() = Some(GarageCycleState {
        last_completed_cycle_distance_meters: Some(25_500.0),
        ..GarageCycleState::new(Some(42), 7)
    });

    assert_eq!(
        GarageCyclePersistenceUseCase::new(store)
            .last_completed_distance_km(7)
            .await
            .unwrap(),
        Some(25.5)
    );
    assert_eq!(
        GarageCyclePersistenceUseCase::new(MemoryCycleStore::default())
            .last_completed_distance_km(7)
            .await
            .unwrap(),
        None
    );
}
