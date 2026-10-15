use business::domain::enums::{PreventiveControlType, PreventiveStatus};
use business::domain::preventive_plan::PreventivePlan;
use business::domain::tenant_rule_setting::RuleSettings;
use business::use_cases::garage_queue_use_case::worst_maintenance_status;

fn plan(interval_km: f64) -> PreventivePlan {
    PreventivePlan {
        id: None,
        uuid: None,
        tenant_id: Some(1),
        vehicle_id: 7,
        plan_name: format!("service-{interval_km}"),
        control_type: PreventiveControlType::Kilometers,
        interval_km: Some(interval_km),
        interval_days: None,
        last_service_km: Some(0.0),
        last_service_date: None,
        extension_limit_km: None,
        last_work_order_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[test]
fn monitor_uses_the_worst_preventive_plan_status() {
    let plans = [plan(20_000.0), plan(10_000.0)];

    assert_eq!(
        worst_maintenance_status(
            &plans,
            Some(9_500.0),
            chrono::NaiveDate::from_ymd_opt(2026, 10, 13).unwrap(),
            &RuleSettings::default(),
        ),
        Some(PreventiveStatus::Attention)
    );
    assert_eq!(
        worst_maintenance_status(
            &plans,
            Some(10_000.0),
            chrono::NaiveDate::from_ymd_opt(2026, 10, 13).unwrap(),
            &RuleSettings::default(),
        ),
        Some(PreventiveStatus::Overdue)
    );
}

#[test]
fn monitor_omits_maintenance_status_when_no_plan_applies() {
    assert_eq!(
        worst_maintenance_status(
            &[],
            Some(10_000.0),
            chrono::NaiveDate::from_ymd_opt(2026, 10, 13).unwrap(),
            &RuleSettings::default(),
        ),
        None
    );
}
