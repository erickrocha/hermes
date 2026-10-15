//! `EPIC-GA-06-S03` (`HRMS-970`): hermes's first background job -- the garage's day
//! boundary (`TRM-419…422`, `426`).
//!
//! Scope (`D-05`/`HRMS-903`): the tenant list is read with the explicit platform
//! grant, and each tenant's pass runs under `run_for_tenant`, so a pass can see and
//! write that tenant's rows and nothing wider.
//!
//! Every instance runs its own loop, and the pass is idempotent, so two instances
//! at once do no harm (ponytail: no leader election; add one if passes get costly).

use business::commons::gateway::Gateway;
use business::gateway::customer_day_off_gateway::CustomerDayOffGateway;
use business::gateway::daily_schedule_gateway::DailyScheduleGateway;
use business::gateway::effective_schedule_gateway::EffectiveScheduleDatabaseGateway;
use business::gateway::extra_trip_gateway::ExtraTripGateway;
use business::gateway::fuel_entry_gateway::FuelEntryGateway;
use business::gateway::fuel_gauge_setting_gateway::FuelGaugeSettingGateway;
use business::gateway::garage_attendance_gateway::GarageAttendanceGateway;
use business::gateway::garage_auto_departure_gateway::GarageAutoDepartureDatabaseGateway;
use business::gateway::garage_rollover_gateway::GarageRolloverGateway;
use business::gateway::garage_service_model_gateway::GarageServiceModelGateway;
use business::gateway::holiday_gateway::HolidayGateway;
use business::gateway::km_evolution_gateway::KmEvolutionGateway;
use business::gateway::schedule_exception_gateway::ScheduleExceptionGateway;
use business::gateway::tenant_gateway::TenantGateway;
use business::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway;
use business::gateway::transport_demand_allocation_gateway::TransportDemandAllocationGateway;
use business::gateway::transport_demand_gateway::TransportDemandGateway;
use business::gateway::vehicle_gateway::VehicleGateway;
use business::gateway::vehicle_presence_event_gateway::VehiclePresenceEventGateway;
use business::sea_orm::DbConn;
use business::use_cases::effective_schedule_use_case::EffectiveScheduleUseCase;
use business::use_cases::fuel_gauge_use_case::FuelGaugeUseCase;
use business::use_cases::garage_auto_departure_use_case::GarageAutoDepartureUseCase;
use business::use_cases::garage_rollover_use_case::GarageRolloverUseCase;
use business::use_cases::garage_validity_use_case::GarageValidityUseCase;
use business::use_cases::vehicle_presence_use_case::VehiclePresenceUseCase;
use std::time::Duration;

const ACTOR: &str = "garage-rollover";
const AUTO_DEPARTURE_ACTOR: &str = "garage-auto-departure";

/// One pass over every tenant. A tenant that fails is logged and the rest still run.
pub async fn run_rollover_once(db: &DbConn) {
    let tenants =
        match entity::audit::run_as_platform(TenantGateway::new(db.clone()).find_all()).await {
            Ok(tenants) => tenants,
            Err(e) => return log::error!("[{ACTOR}] could not list tenants: {e}"),
        };
    for tenant in tenants {
        let use_case = GarageRolloverUseCase::new(
            GarageRolloverGateway::new(
                GarageAttendanceGateway::new(db.clone()),
                TenantRuleSettingGateway::new(db.clone()),
            ),
            VehiclePresenceUseCase::new(
                VehiclePresenceEventGateway::new(db.clone()),
                VehicleGateway::new(db.clone()),
            ),
        );
        match entity::audit::run_for_tenant(tenant.id, ACTOR, use_case.run(Some(tenant.id))).await {
            Ok(o) if o.closed + o.redated > 0 => {
                log::info!(
                    "[{ACTOR}] tenant {}: {} closed, {} re-dated",
                    tenant.id,
                    o.closed,
                    o.redated
                )
            }
            Ok(_) => {}
            Err(e) => log::error!("[{ACTOR}] tenant {}: {}", tenant.id, e.message),
        }
    }
}

/// Starts the loop. `GARAGE_ROLLOVER_INTERVAL_SECONDS` sets the period (default
/// 900); `0` turns the job off.
pub fn spawn_rollover(db: DbConn) {
    let seconds: u64 = std::env::var("GARAGE_ROLLOVER_INTERVAL_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(900);
    if seconds == 0 {
        return log::info!("[{ACTOR}] disabled (GARAGE_ROLLOVER_INTERVAL_SECONDS=0)");
    }
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(seconds));
        loop {
            tick.tick().await;
            run_rollover_once(&db).await;
        }
    });
}

pub async fn run_auto_departure_once(db: &DbConn) {
    let tenants =
        match entity::audit::run_as_platform(TenantGateway::new(db.clone()).find_all()).await {
            Ok(tenants) => tenants,
            Err(e) => return log::error!("[{AUTO_DEPARTURE_ACTOR}] could not list tenants: {e}"),
        };
    for tenant in tenants {
        let departure = GarageAutoDepartureUseCase::new(
            GarageAutoDepartureDatabaseGateway::new(db.clone()),
            EffectiveScheduleUseCase::new(EffectiveScheduleDatabaseGateway::new(
                TransportDemandGateway::new(db.clone()),
                TransportDemandAllocationGateway::new(db.clone()),
                DailyScheduleGateway::new(db.clone()),
                ScheduleExceptionGateway::new(db.clone()),
                HolidayGateway::new(db.clone()),
                CustomerDayOffGateway::new(db.clone()),
                ExtraTripGateway::new(db.clone()),
            )),
            GarageValidityUseCase::new(
                VehiclePresenceEventGateway::new(db.clone()),
                ExtraTripGateway::new(db.clone()),
                GarageServiceModelGateway::new(db.clone()),
                TenantRuleSettingGateway::new(db.clone()),
                FuelGaugeUseCase::new(
                    FuelEntryGateway::new(db.clone()),
                    VehicleGateway::new(db.clone()),
                    KmEvolutionGateway::new(db.clone()),
                    FuelGaugeSettingGateway::new(db.clone()),
                    TenantRuleSettingGateway::new(db.clone()),
                ),
            ),
        );
        match entity::audit::run_for_tenant(tenant.id, AUTO_DEPARTURE_ACTOR, departure.run()).await
        {
            Ok(outcome) if outcome.closed > 0 => log::info!(
                "[{AUTO_DEPARTURE_ACTOR}] tenant {}: {} departures closed",
                tenant.id,
                outcome.closed
            ),
            Ok(_) => {}
            Err(e) => log::error!(
                "[{AUTO_DEPARTURE_ACTOR}] tenant {}: {}",
                tenant.id,
                e.message
            ),
        }
    }
}

pub fn spawn_auto_departure(db: DbConn) {
    let seconds: u64 = std::env::var("GARAGE_AUTO_DEPARTURE_INTERVAL_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    if seconds == 0 {
        return log::info!(
            "[{AUTO_DEPARTURE_ACTOR}] disabled (GARAGE_AUTO_DEPARTURE_INTERVAL_SECONDS=0)"
        );
    }
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(seconds));
        loop {
            tick.tick().await;
            run_auto_departure_once(&db).await;
        }
    });
}
