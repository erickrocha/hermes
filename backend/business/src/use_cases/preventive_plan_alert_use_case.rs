use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::preventive_plan::{PreventivePlan, PreventivePlanEntityMapper};
use crate::domain::preventive_plan_alert::{PreventivePlanAlert, PreventivePlanAlertEntityMapper};
use crate::gateway::preventive_plan_alert_gateway::PreventivePlanAlertGateway;
use crate::gateway::preventive_plan_gateway::PreventivePlanGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use sea_orm::DbErr;

pub const PLAN_NOT_FOUND: &str = "Preventive plan not found";
pub const ALERT_NOT_FOUND: &str = "Alert not found";
pub const ALERT_INVALID: &str = "An alert needs a positive kilometre distance, a title and an inspection model";
pub const DUPLICATE_ALERT: &str = "This plan already has an alert at this kilometre distance";
pub const ALERT_NOT_PENDING: &str = "This alert is not pending in the plan's current cycle";

/// `EPIC-MT-07-S08` (`HRMS-713`, `TRM-323…325`): intermediate inspection
/// alerts on a preventive plan.
pub struct PreventivePlanAlertUseCase {
    alerts: PreventivePlanAlertGateway,
    plans: PreventivePlanGateway,
    vehicles: VehicleGateway,
}

impl PreventivePlanAlertUseCase {
    pub fn new(alerts: PreventivePlanAlertGateway, plans: PreventivePlanGateway, vehicles: VehicleGateway) -> Self {
        Self { alerts, plans, vehicles }
    }

    /// `TRM-323`: declares an alert `at_km` kilometres into the plan's cycle.
    pub async fn add(
        &self,
        plan_uuid: String,
        at_km: f64,
        title: String,
        inspection_model: String,
    ) -> Result<PreventivePlanAlert, BusinessError> {
        let (title, inspection_model) = (title.trim().to_string(), inspection_model.trim().to_string());
        if at_km.is_nan() || at_km <= 0.0 || title.is_empty() || inspection_model.is_empty() {
            return Err(BusinessError::new(ALERT_INVALID.to_string()));
        }
        let plan = self.plan(plan_uuid).await?;
        let plan_id = plan.id.unwrap_or_default();
        let existing = self.alerts.find_by_plan(plan_id).await.map_err(database_error)?;
        if existing.iter().any(|a| a.at_km == at_km) {
            return Err(BusinessError::new(DUPLICATE_ALERT.to_string()));
        }
        let entity = self
            .alerts
            .persist(PreventivePlanAlert {
                id: None,
                uuid: None,
                tenant_id: plan.tenant_id,
                preventive_plan_id: plan_id,
                at_km,
                title,
                inspection_model,
                discharged_cycle: None,
                created_at: None,
                created_by: None,
                updated_at: None,
                updated_by: None,
            })
            .await
            .map_err(database_error)?;
        Ok(PreventivePlanAlertEntityMapper::from_active_model(entity))
    }

    /// `TRM-324`: the alerts reached in the current cycle and not yet
    /// discharged, ascending by kilometre.
    pub async fn pending(&self, plan_uuid: String) -> Result<Vec<PreventivePlanAlert>, BusinessError> {
        let plan = self.plan(plan_uuid).await?;
        let alerts = self.alerts.find_by_plan(plan.id.unwrap_or_default()).await.map_err(database_error)?;
        let current_km = self.current_km(&plan).await?;
        Ok(pending_alerts(&plan, PreventivePlanAlertEntityMapper::from_models(alerts), current_km))
    }

    /// `TRM-325`: discharges a pending alert for the plan's current cycle; a
    /// real service changes the cycle key and so reopens it.
    pub async fn discharge(&self, alert_uuid: String) -> Result<PreventivePlanAlert, BusinessError> {
        let alert = self
            .alerts
            .find_by_uuid(alert_uuid)
            .await
            .map_err(database_error)?
            .map(PreventivePlanAlertEntityMapper::from_model)
            .ok_or_else(|| BusinessError::new(ALERT_NOT_FOUND.to_string()))?;
        let plan = self
            .plans
            .find_by_id(alert.preventive_plan_id)
            .await
            .map_err(database_error)?
            .map(PreventivePlanEntityMapper::from_model)
            .ok_or_else(|| BusinessError::new(PLAN_NOT_FOUND.to_string()))?;
        let current_km = self.current_km(&plan).await?;
        if pending_alerts(&plan, vec![alert.clone()], current_km).is_empty() {
            return Err(BusinessError::new(ALERT_NOT_PENDING.to_string()));
        }
        let entity = self
            .alerts
            .persist(PreventivePlanAlert { discharged_cycle: Some(cycle_key(&plan)), ..alert })
            .await
            .map_err(database_error)?;
        Ok(PreventivePlanAlertEntityMapper::from_active_model(entity))
    }

    async fn plan(&self, uuid: String) -> Result<PreventivePlan, BusinessError> {
        self.plans
            .find_by_uuid(uuid)
            .await
            .map_err(database_error)?
            .map(PreventivePlanEntityMapper::from_model)
            .ok_or_else(|| BusinessError::new(PLAN_NOT_FOUND.to_string()))
    }

    async fn current_km(&self, plan: &PreventivePlan) -> Result<Option<f64>, BusinessError> {
        Ok(self
            .vehicles
            .find_by_id(plan.vehicle_id)
            .await
            .map_err(database_error)?
            .and_then(|v| v.odometer_km))
    }
}

/// `TRM-325`: the cycle changes when a real service moves the baseline.
pub fn cycle_key(plan: &PreventivePlan) -> String {
    let date = plan.last_service_date.map(|d| d.to_string()).unwrap_or_default();
    format!("{}|{}", plan.last_service_km.unwrap_or(0.0), date)
}

/// `TRM-324`: reached (kilometres driven since the last service >= threshold)
/// and not discharged for this cycle, ascending by kilometre. Without a known
/// odometer nothing is reached.
pub fn pending_alerts(plan: &PreventivePlan, alerts: Vec<PreventivePlanAlert>, current_km: Option<f64>) -> Vec<PreventivePlanAlert> {
    let Some(current) = current_km else { return vec![] };
    let driven = (current - plan.last_service_km.unwrap_or(0.0)).max(0.0);
    let cycle = cycle_key(plan);
    let mut pending: Vec<_> = alerts
        .into_iter()
        .filter(|a| driven >= a.at_km && a.discharged_cycle.as_deref() != Some(cycle.as_str()))
        .collect();
    pending.sort_by(|a, b| a.at_km.total_cmp(&b.at_km));
    pending
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[PreventivePlanAlertUseCase] {}", msg);
    BusinessError::new(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::enums::PreventiveControlType;
    use chrono::NaiveDate;

    fn plan(last_km: f64, last_date: (i32, u32, u32)) -> PreventivePlan {
        PreventivePlan {
            id: Some(1),
            uuid: None,
            tenant_id: Some(1),
            vehicle_id: 1,
            plan_name: "Oil".into(),
            control_type: PreventiveControlType::Kilometers,
            interval_km: Some(20_000.0),
            interval_days: None,
            last_service_km: Some(last_km),
            last_service_date: NaiveDate::from_ymd_opt(last_date.0, last_date.1, last_date.2),
            extension_limit_km: None,
            last_work_order_id: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }
    }

    fn alert(at_km: f64, discharged: Option<String>) -> PreventivePlanAlert {
        PreventivePlanAlert {
            id: None,
            uuid: None,
            tenant_id: Some(1),
            preventive_plan_id: 1,
            at_km,
            title: "Brakes".into(),
            inspection_model: "Brake check".into(),
            discharged_cycle: discharged,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }
    }

    #[test]
    fn an_alert_is_raised_once_the_kilometres_since_service_reach_it_in_ascending_order() {
        let p = plan(50_000.0, (2026, 1, 1));
        let alerts = vec![alert(15_000.0, None), alert(5_000.0, None), alert(10_000.0, None)];
        // 12,000 km driven: 5,000 and 10,000 reached, 15,000 not yet.
        let pending = pending_alerts(&p, alerts, Some(62_000.0));
        assert_eq!(pending.iter().map(|a| a.at_km).collect::<Vec<_>>(), vec![5_000.0, 10_000.0]);
    }

    #[test]
    fn a_discharged_alert_stays_quiet_until_a_real_service_reopens_it() {
        let p = plan(50_000.0, (2026, 1, 1));
        let discharged = vec![alert(5_000.0, Some(cycle_key(&p)))];
        assert!(pending_alerts(&p, discharged.clone(), Some(56_000.0)).is_empty());
        // A real service moves the baseline: new cycle key, alert reopens once reached again.
        let serviced = plan(70_000.0, (2026, 6, 1));
        assert!(pending_alerts(&serviced, discharged.clone(), Some(74_000.0)).is_empty());
        assert_eq!(pending_alerts(&serviced, discharged, Some(76_000.0)).len(), 1);
    }

    #[test]
    fn without_an_odometer_nothing_is_reached() {
        let p = plan(50_000.0, (2026, 1, 1));
        assert!(pending_alerts(&p, vec![alert(1.0, None)], None).is_empty());
    }
}
