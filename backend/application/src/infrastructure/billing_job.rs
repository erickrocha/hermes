//! Closes subscription billing cycles. Tenant enumeration uses an explicit
//! platform grant; all reads and writes for an individual cycle run tenant-scoped.

use business::commons::gateway::Gateway;
use business::gateway::billing_gateway::BillingGateway;
use business::gateway::business_plan_gateway::BusinessPlanGateway;
use business::gateway::tenant_gateway::TenantGateway;
use business::sea_orm::DbConn;
use business::use_cases::billing_use_case::BillingUseCase;
use chrono::Utc;
use std::time::Duration;

const ACTOR: &str = "billing-cycle";

fn use_case(db: &DbConn) -> BillingUseCase<BillingGateway, TenantGateway, BusinessPlanGateway> {
    BillingUseCase::new(
        BillingGateway::new(db.clone()),
        TenantGateway::new(db.clone()),
        BusinessPlanGateway::new(db.clone()),
    )
}

pub async fn close_due_cycles_once(db: &DbConn) {
    let tenants = match entity::audit::run_as_platform(TenantGateway::new(db.clone()).find_all()).await {
        Ok(tenants) => tenants,
        Err(error) => return log::error!("[{ACTOR}] could not list tenants: {error}"),
    };

    for tenant in tenants.into_iter().filter(|tenant| tenant.business_plan_id.is_some()) {
        let tenant_id = tenant.id;
        let use_case = use_case(db);
        let pass = async {
            match use_case.get_settings(tenant_id).await? {
                Some(settings) if settings.grace_days.is_some() && settings.cycle_started_at.is_some() => {},
                _ => {
                    log::warn!("[{ACTOR}] tenant {tenant_id} is not billing-configured; set its grace days before its first cycle");
                    return Ok(None);
                }
            }
            use_case.close_due_cycle(tenant_id, Utc::now().naive_utc()).await
        };

        match entity::audit::run_for_tenant(tenant_id, ACTOR, pass).await {
            Ok(Some(charge)) => log::info!(
                "[{ACTOR}] tenant {tenant_id}: closed cycle ending {}; charge {} BRL cents",
                charge.period_end,
                charge.total_in_cents
            ),
            Ok(None) => {}
            Err(error) => log::error!("[{ACTOR}] tenant {tenant_id}: {}", error.message),
        }
    }
}

/// `BILLING_CYCLE_CHECK_INTERVAL_SECONDS` controls cadence (default 300); zero disables it.
pub fn spawn(db: DbConn) {
    let seconds: u64 = std::env::var("BILLING_CYCLE_CHECK_INTERVAL_SECONDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(300);
    if seconds == 0 {
        return log::info!("[{ACTOR}] disabled (BILLING_CYCLE_CHECK_INTERVAL_SECONDS=0)");
    }
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(seconds));
        loop {
            tick.tick().await;
            close_due_cycles_once(&db).await;
        }
    });
}