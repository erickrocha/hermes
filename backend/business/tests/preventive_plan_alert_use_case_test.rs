use business::domain::business_error::BusinessError;
use business::domain::enums::{PreventiveControlType, VehicleStatus};
use business::domain::preventive_plan::PreventivePlan;
use business::domain::preventive_plan_alert::PreventivePlanAlert;
use business::domain::vehicle::Vehicle;
use business::gateway::preventive_plan_alert_gateway::PreventivePlanAlertStore;
use business::gateway::preventive_plan_gateway::PreventivePlanLookupStore;
use business::gateway::vehicle_gateway::VehicleLookupStore;
use business::use_cases::preventive_plan_alert_use_case::{
    ALERT_NOT_PENDING, DUPLICATE_ALERT, PreventivePlanAlertUseCase,
};
use chrono::NaiveDate;
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryAlertStore {
    alerts: Mutex<Vec<PreventivePlanAlert>>,
}

#[async_trait]
impl PreventivePlanAlertStore for MemoryAlertStore {
    async fn save(
        &self,
        mut alert: PreventivePlanAlert,
    ) -> Result<PreventivePlanAlert, BusinessError> {
        let mut alerts = self
            .alerts
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let id = alert.id.unwrap_or(alerts.len() as i64 + 1);
        alert.id = Some(id);
        alert.uuid = Some(format!("preventive-alert-{id}"));
        if let Some(existing) = alerts.iter_mut().find(|existing| existing.id == Some(id)) {
            *existing = alert.clone();
        } else {
            alerts.push(alert.clone());
        }
        Ok(alert)
    }

    async fn get_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<PreventivePlanAlert>, BusinessError> {
        let alerts = self
            .alerts
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(alerts
            .iter()
            .find(|alert| alert.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }

    async fn list_by_plan(&self, plan_id: i64) -> Result<Vec<PreventivePlanAlert>, BusinessError> {
        let alerts = self
            .alerts
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(alerts
            .iter()
            .filter(|alert| alert.preventive_plan_id == plan_id)
            .cloned()
            .collect())
    }
}

struct MemoryPlanStore {
    plan: Option<PreventivePlan>,
}

#[async_trait]
impl PreventivePlanLookupStore for MemoryPlanStore {
    async fn get_by_id(&self, id: i64) -> Result<Option<PreventivePlan>, BusinessError> {
        Ok(self
            .plan
            .as_ref()
            .filter(|plan| plan.id == Some(id))
            .cloned())
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<PreventivePlan>, BusinessError> {
        Ok(self
            .plan
            .as_ref()
            .filter(|plan| plan.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }
}

struct MemoryVehicleStore {
    vehicle: Option<Vehicle>,
}

#[async_trait]
impl VehicleLookupStore for MemoryVehicleStore {
    async fn get_by_id(&self, id: i64) -> Result<Option<Vehicle>, BusinessError> {
        Ok(self
            .vehicle
            .as_ref()
            .filter(|vehicle| vehicle.id == Some(id))
            .cloned())
    }
}

fn plan() -> PreventivePlan {
    PreventivePlan {
        id: Some(1),
        uuid: Some("plan-1".into()),
        tenant_id: Some(42),
        vehicle_id: 10,
        plan_name: "Oil service".into(),
        control_type: PreventiveControlType::Kilometers,
        interval_km: Some(20_000.0),
        interval_days: None,
        last_service_km: Some(50_000.0),
        last_service_date: Some(NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid test date")),
        extension_limit_km: None,
        last_work_order_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn vehicle(odometer_km: Option<f64>) -> Vehicle {
    Vehicle {
        id: Some(10),
        uuid: Some("vehicle-10".into()),
        tenant_id: Some(42),
        plate: "ABC1D23".into(),
        model: "Truck".into(),
        status: VehicleStatus::Active,
        tracker_device_id: None,
        prefix: None,
        vehicle_type: None,
        odometer_km,
        wheel_type: None,
        spare_tire_count: None,
        spare_tire_type: None,
        spare_tire_notes: None,
        tank_capacity_liters: None,
        reference_km_per_liter: None,
        garage_tag: None,
        garage_tag_origin: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn use_case(odometer_km: Option<f64>) -> PreventivePlanAlertUseCase {
    PreventivePlanAlertUseCase::new(
        MemoryAlertStore::default(),
        MemoryPlanStore { plan: Some(plan()) },
        MemoryVehicleStore {
            vehicle: Some(vehicle(odometer_km)),
        },
    )
}

#[tokio::test]
async fn alerts_become_pending_at_their_kilometre_threshold_and_discharge_by_cycle() {
    let use_case = use_case(Some(56_000.0));
    let alert = use_case
        .add(
            "plan-1".into(),
            5_000.0,
            " Brakes ".into(),
            " Brake inspection ".into(),
        )
        .await
        .expect("alert is added");
    let pending = use_case
        .pending("plan-1".into())
        .await
        .expect("pending alerts are listed");
    let discharged = use_case
        .discharge(alert.uuid.clone().expect("store assigned uuid"))
        .await
        .expect("pending alert is discharged");
    let pending_after = use_case
        .pending("plan-1".into())
        .await
        .expect("pending alerts are listed again");

    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].title, "Brakes");
    assert!(discharged.discharged_cycle.is_some());
    assert!(pending_after.is_empty());
}

#[tokio::test]
async fn duplicate_alert_distance_is_rejected() {
    let use_case = use_case(Some(56_000.0));
    use_case
        .add(
            "plan-1".into(),
            5_000.0,
            "Brakes".into(),
            "Inspection".into(),
        )
        .await
        .expect("first alert is added");

    let error = use_case
        .add(
            "plan-1".into(),
            5_000.0,
            "Tires".into(),
            "Inspection".into(),
        )
        .await
        .expect_err("duplicate kilometre is refused");

    assert_eq!(error.message, DUPLICATE_ALERT);
}

#[tokio::test]
async fn an_alert_not_reached_by_the_odometer_cannot_be_discharged() {
    let use_case = use_case(Some(53_000.0));
    let alert = use_case
        .add(
            "plan-1".into(),
            5_000.0,
            "Brakes".into(),
            "Inspection".into(),
        )
        .await
        .expect("alert is added");

    let error = use_case
        .discharge(alert.uuid.expect("store assigned uuid"))
        .await
        .expect_err("unreached alert is not pending");

    assert_eq!(error.message, ALERT_NOT_PENDING);
}
