use business::domain::business_error::BusinessError;
use business::domain::maintenance_notification::MaintenanceNotification;
use business::gateway::maintenance_notification_gateway::MaintenanceNotificationStore;
use business::use_cases::maintenance_notification_use_case::{
    MAINTENANCE_NOTIFICATION_NOT_FOUND, MaintenanceNotificationUseCase,
};
use chrono::{NaiveDate, NaiveDateTime};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::{Arc, Mutex};

/// In-memory store that, like the real gateway, only shows the caller's own tenant.
#[derive(Clone)]
struct MemoryStore {
    rows: Arc<Mutex<Vec<MaintenanceNotification>>>,
    caller_tenant: i64,
}

impl MemoryStore {
    fn new(caller_tenant: i64, rows: Arc<Mutex<Vec<MaintenanceNotification>>>) -> Self {
        Self { rows, caller_tenant }
    }

    fn visible(&self) -> Vec<MaintenanceNotification> {
        let rows = self.rows.lock().expect("lock");
        rows.iter()
            .filter(|n| n.tenant_id == Some(self.caller_tenant))
            .cloned()
            .collect()
    }
}

#[async_trait]
impl MaintenanceNotificationStore for MemoryStore {
    async fn save(
        &self,
        mut n: MaintenanceNotification,
    ) -> Result<MaintenanceNotification, BusinessError> {
        let mut rows = self.rows.lock().expect("lock");
        if n.id.is_none() {
            n.id = Some(rows.len() as i64 + 1);
            n.uuid = Some(format!("uuid-{}", rows.len() + 1));
            rows.push(n.clone());
        } else if let Some(slot) = rows.iter_mut().find(|r| r.id == n.id) {
            *slot = n.clone();
        }
        Ok(n)
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<MaintenanceNotification>, BusinessError> {
        Ok(self
            .visible()
            .into_iter()
            .find(|n| n.uuid.as_deref() == Some(uuid.as_str())))
    }

    async fn find_page(
        &self,
        unread_only: bool,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<MaintenanceNotification>, u64), BusinessError> {
        let mut all: Vec<_> = self
            .visible()
            .into_iter()
            .filter(|n| !unread_only || n.read_at.is_none())
            .collect();
        all.reverse();
        let total = all.len() as u64;
        let items = all
            .into_iter()
            .skip((page * page_size) as usize)
            .take(page_size as usize)
            .collect();
        Ok((items, total))
    }

    async fn count_unread(&self) -> Result<u64, BusinessError> {
        Ok(self.visible().iter().filter(|n| n.read_at.is_none()).count() as u64)
    }

    async fn mark_all_read(&self, now: NaiveDateTime) -> Result<u64, BusinessError> {
        let mut rows = self.rows.lock().expect("lock");
        let mut marked = 0;
        for n in rows
            .iter_mut()
            .filter(|n| n.tenant_id == Some(self.caller_tenant) && n.read_at.is_none())
        {
            n.read_at = Some(now);
            marked += 1;
        }
        Ok(marked)
    }
}

fn notice(tenant: i64, part_name: &str) -> MaintenanceNotification {
    MaintenanceNotification {
        id: None,
        uuid: None,
        tenant_id: Some(tenant),
        work_order_id: 5,
        part_id: 7,
        part_name: part_name.to_string(),
        quantity: 3.0,
        read_at: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn at(day: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 10, day)
        .and_then(|d| d.and_hms_opt(9, 0, 0))
        .expect("valid date")
}

fn shared() -> Arc<Mutex<Vec<MaintenanceNotification>>> {
    Arc::new(Mutex::new(Vec::new()))
}

#[tokio::test]
async fn creating_raises_an_unread_notice_even_if_the_caller_sent_a_read_at() {
    let rows = shared();
    let uc = MaintenanceNotificationUseCase::new(MemoryStore::new(1, rows));
    let saved = uc
        .create(MaintenanceNotification {
            read_at: Some(at(1)),
            ..notice(1, "Filtro")
        })
        .await
        .expect("saved");
    assert!(saved.read_at.is_none());
    assert_eq!(saved.part_name, "Filtro");
    assert_eq!(saved.quantity, 3.0);
}

#[tokio::test]
async fn listing_filters_unread_and_counts_them() {
    let uc = MaintenanceNotificationUseCase::new(MemoryStore::new(1, shared()));
    let first = uc.create(notice(1, "A")).await.expect("a");
    uc.create(notice(1, "B")).await.expect("b");
    uc.mark_read(first.uuid.expect("uuid"), at(2)).await.expect("read");

    assert_eq!(uc.count_unread().await.expect("count"), 1);
    let (all, total) = uc.find_page(false, 0, 25).await.expect("all");
    assert_eq!((all.len(), total), (2, 2));
    let (unread, total) = uc.find_page(true, 0, 25).await.expect("unread");
    assert_eq!((unread.len(), total), (1, 1));
    assert_eq!(unread[0].part_name, "B");
}

#[tokio::test]
async fn marking_read_twice_keeps_the_first_read_time() {
    let uc = MaintenanceNotificationUseCase::new(MemoryStore::new(1, shared()));
    let saved = uc.create(notice(1, "A")).await.expect("saved");
    let uuid = saved.uuid.expect("uuid");
    let once = uc.mark_read(uuid.clone(), at(2)).await.expect("once");
    let twice = uc.mark_read(uuid, at(3)).await.expect("twice");
    assert_eq!(once.read_at, Some(at(2)));
    assert_eq!(twice.read_at, Some(at(2)));
}

#[tokio::test]
async fn marking_all_read_reports_how_many_changed() {
    let uc = MaintenanceNotificationUseCase::new(MemoryStore::new(1, shared()));
    uc.create(notice(1, "A")).await.expect("a");
    uc.create(notice(1, "B")).await.expect("b");
    assert_eq!(uc.mark_all_read(at(2)).await.expect("marked"), 2);
    assert_eq!(uc.mark_all_read(at(3)).await.expect("again"), 0);
    assert_eq!(uc.count_unread().await.expect("count"), 0);
}

#[tokio::test]
async fn another_tenant_cannot_read_or_touch_the_notice() {
    let rows = shared();
    let owner = MaintenanceNotificationUseCase::new(MemoryStore::new(1, rows.clone()));
    let other = MaintenanceNotificationUseCase::new(MemoryStore::new(2, rows));
    let saved = owner.create(notice(1, "A")).await.expect("saved");
    let uuid = saved.uuid.expect("uuid");

    let refused = other.mark_read(uuid.clone(), at(2)).await.expect_err("hidden");
    assert_eq!(refused.message, MAINTENANCE_NOTIFICATION_NOT_FOUND);
    assert_eq!(other.count_unread().await.expect("count"), 0);
    assert_eq!(other.mark_all_read(at(2)).await.expect("none"), 0);
    // The owner's notice is still unread.
    assert_eq!(owner.count_unread().await.expect("count"), 1);
}
