use crate::domain::business_error::BusinessError;
use crate::domain::maintenance_notification::MaintenanceNotification;
use crate::gateway::maintenance_notification_gateway::MaintenanceNotificationStore;
use chrono::NaiveDateTime;

pub const MAINTENANCE_NOTIFICATION_NOT_FOUND: &str = "Maintenance notification not found";

/// `EPIC-MT-06` (`C-026`, `TRM-616`): the in-app inbox of "the part you were waiting for arrived".
pub struct MaintenanceNotificationUseCase {
    gateway: Box<dyn MaintenanceNotificationStore>,
}

impl MaintenanceNotificationUseCase {
    pub fn new<S>(gateway: S) -> Self
    where
        S: MaintenanceNotificationStore + 'static,
    {
        Self {
            gateway: Box::new(gateway),
        }
    }

    /// Raises one unread notice; whatever `read_at` the caller sent is dropped.
    pub async fn create(
        &self,
        notification: MaintenanceNotification,
    ) -> Result<MaintenanceNotification, BusinessError> {
        log::info!(
            "[MaintenanceNotificationUseCase] notifying work order {} about part {}",
            notification.work_order_id,
            notification.part_id
        );
        self.gateway
            .save(MaintenanceNotification {
                id: None,
                uuid: None,
                read_at: None,
                ..notification
            })
            .await
    }

    pub async fn find_page(
        &self,
        unread_only: bool,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<MaintenanceNotification>, u64), BusinessError> {
        self.gateway.find_page(unread_only, page, page_size).await
    }

    pub async fn count_unread(&self) -> Result<u64, BusinessError> {
        self.gateway.count_unread().await
    }

    pub async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<MaintenanceNotification, BusinessError> {
        self.gateway
            .find_by_uuid(uuid)
            .await?
            .ok_or_else(|| BusinessError::new(MAINTENANCE_NOTIFICATION_NOT_FOUND.to_string()))
    }

    /// Idempotent: an already-read notice is returned untouched, keeping its first `read_at`.
    pub async fn mark_read(
        &self,
        uuid: String,
        now: NaiveDateTime,
    ) -> Result<MaintenanceNotification, BusinessError> {
        let notification = self.find_by_uuid(uuid).await?;
        if notification.read_at.is_some() {
            return Ok(notification);
        }
        self.gateway
            .save(MaintenanceNotification {
                read_at: Some(now),
                ..notification
            })
            .await
    }

    pub async fn mark_all_read(&self, now: NaiveDateTime) -> Result<u64, BusinessError> {
        self.gateway.mark_all_read(now).await
    }
}
