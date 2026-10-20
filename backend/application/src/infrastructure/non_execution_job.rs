//! `EPIC-MT-04` (`SR-MT-04-12`): the background pass that finds planned maintenance that was
//! not carried out. Modelled on `garage_jobs`: the tenant list is read with the explicit
//! platform grant and each tenant's pass runs under that tenant's scope. The pass is
//! idempotent (occurrences have unique business keys), so two instances at once do no harm.

use business::domain::non_execution::EvaluationOutcome;
use business::gateway::non_execution_gateway::NonExecutionGateway;
use business::gateway::tenant_gateway::TenantGateway;
use business::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway;
use business::commons::gateway::Gateway;
use business::sea_orm::DbConn;
use business::use_cases::non_execution_use_case::NonExecutionUseCase;
use chrono::Utc;
use std::time::Duration;

const ACTOR: &str = "non-execution";

pub fn use_case(db: &DbConn) -> NonExecutionUseCase {
    NonExecutionUseCase::new(NonExecutionGateway::new(db.clone()), TenantRuleSettingGateway::new(db.clone()))
}

/// One pass over every tenant; one that fails is logged and the rest still run.
pub async fn evaluate_all_tenants(db: &DbConn) -> EvaluationOutcome {
    let mut total = EvaluationOutcome::default();
    let tenants = match entity::audit::run_as_platform(TenantGateway::new(db.clone()).find_all()).await {
        Ok(tenants) => tenants,
        Err(e) => {
            log::error!("[{ACTOR}] could not list tenants: {e}");
            return total;
        }
    };
    for tenant in tenants {
        let evaluation = use_case(db);
        let pass = evaluation.evaluate(Utc::now().naive_utc());
        match entity::audit::run_for_tenant(tenant.id, ACTOR, pass).await {
            Ok(outcome) => {
                if outcome.occurrences_recorded + outcome.plans_concluded > 0 {
                    log::info!(
                        "[{ACTOR}] tenant {}: {} occurrences, {} plans concluded",
                        tenant.id, outcome.occurrences_recorded, outcome.plans_concluded
                    );
                }
                total.plans_evaluated += outcome.plans_evaluated;
                total.occurrences_recorded += outcome.occurrences_recorded;
                total.plans_concluded += outcome.plans_concluded;
            }
            Err(e) => log::error!("[{ACTOR}] tenant {}: {}", tenant.id, e.message),
        }
    }
    total
}

/// Starts the loop. `NON_EXECUTION_EVALUATION_INTERVAL_SECONDS` sets the period (default 300);
/// `0` turns the job off.
pub fn spawn_evaluation(db: DbConn) {
    let seconds: u64 = std::env::var("NON_EXECUTION_EVALUATION_INTERVAL_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    if seconds == 0 {
        return log::info!("[{ACTOR}] disabled (NON_EXECUTION_EVALUATION_INTERVAL_SECONDS=0)");
    }
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(seconds));
        loop {
            tick.tick().await;
            evaluate_all_tenants(&db).await;
        }
    });
}
