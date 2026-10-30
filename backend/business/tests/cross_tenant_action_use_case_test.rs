use business::domain::business_error::BusinessError;
use business::domain::cross_tenant_action::{CrossTenantAction, KIND_READ, KIND_WRITE};
use business::gateway::cross_tenant_action_gateway::CrossTenantActionStore;
use business::use_cases::cross_tenant_action_use_case::CrossTenantActionUseCase;
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryStore {
    rows: Mutex<Vec<CrossTenantAction>>,
    fail: bool,
}

#[async_trait]
impl CrossTenantActionStore for MemoryStore {
    async fn record(&self, action: CrossTenantAction) -> Result<(), BusinessError> {
        if self.fail {
            return Err(BusinessError::new("disk full".into()));
        }
        self.rows.lock().expect("lock").push(action);
        Ok(())
    }

    async fn list_page(&self, tenant_id: i64, page: u64, page_size: u64) -> Result<(Vec<CrossTenantAction>, u64), BusinessError> {
        let rows = self.rows.lock().expect("lock");
        let mine: Vec<_> = rows.iter().filter(|a| a.affected_tenant_id == tenant_id || a.actor_tenant_id == tenant_id).cloned().collect();
        let total = mine.len() as u64;
        Ok((mine.into_iter().skip((page * page_size) as usize).take(page_size as usize).collect(), total))
    }
}

fn action(actor_tenant: i64, affected_tenant: i64, method: &str) -> CrossTenantAction {
    CrossTenantAction {
        id: None,
        uuid: None,
        actor_user_id: 1,
        actor_email: "support@example.test".into(),
        actor_tenant_id: actor_tenant,
        affected_tenant_id: affected_tenant,
        kind: CrossTenantAction::kind_of(method).to_string(),
        method: method.into(),
        route: "/vehicle/uuid/{uuid}".into(),
        path: "/vehicle/uuid/abc".into(),
        occurred_at: chrono::NaiveDate::from_ymd_opt(2026, 10, 29).expect("date").and_hms_opt(12, 0, 0).expect("time"),
    }
}

/// `HRMS-877`: a safe method is a read, anything else a write.
#[test]
fn a_safe_method_reads_and_every_other_writes() {
    assert_eq!(CrossTenantAction::kind_of("GET"), KIND_READ);
    assert_eq!(CrossTenantAction::kind_of("PUT"), KIND_WRITE);
    assert_eq!(CrossTenantAction::kind_of("DELETE"), KIND_WRITE);
}

/// `SR-AC-08-05`: the affected tenant sees what was done in it, the actor what it did, a stranger nothing.
#[tokio::test]
async fn the_affected_tenant_and_the_actor_see_the_action_and_a_stranger_does_not() {
    let use_case = CrossTenantActionUseCase::new(MemoryStore::default());
    use_case.record(action(1, 2, "PUT")).await.expect("recorded");
    use_case.record(action(1, 3, "GET")).await.expect("recorded");
    assert_eq!(use_case.find_page(2, 0, 10).await.expect("page").1, 1);
    assert_eq!(use_case.find_page(1, 0, 10).await.expect("page").1, 2);
    assert_eq!(use_case.find_page(9, 0, 10).await.expect("page").1, 0);
}

/// `SR-AC-08-06`: a record that cannot be written is an error, so the caller does not carry the action out.
#[tokio::test]
async fn a_record_that_cannot_be_written_is_an_error() {
    let use_case = CrossTenantActionUseCase::new(MemoryStore { fail: true, ..MemoryStore::default() });
    assert!(use_case.record(action(1, 2, "PUT")).await.is_err());
}
