use crate::domain::business_error::BusinessError;
use crate::domain::tenant_dashboard::{PendingChargeWindow, TenantDashboard};
use crate::gateway::tenant_dashboard_gateway::TenantDashboardStore;
use chrono::{Duration, NaiveDateTime};

pub struct TenantDashboardUseCase {
    gateway: Box<dyn TenantDashboardStore>,
}

impl TenantDashboardUseCase {
    pub fn new<S>(gateway: S) -> Self
    where
        S: TenantDashboardStore + 'static,
    {
        Self {
            gateway: Box::new(gateway),
        }
    }

    /// `include_billing` is the tenant owner's view. Other tenant roles still
    /// get the operational counts, without a read of the charge table.
    pub async fn snapshot(
        &self,
        now: NaiveDateTime,
        include_billing: bool,
    ) -> Result<TenantDashboard, BusinessError> {
        let snapshot = TenantDashboard {
            active_vehicles: self.gateway.active_vehicles().await?,
            vehicles_in_maintenance: self.gateway.vehicles_in_maintenance().await?,
            open_work_orders: self.gateway.open_work_orders().await?,
            work_orders_awaiting_parts: self.gateway.work_orders_awaiting_parts().await?,
            yard_queue: self.gateway.yard_queue().await?,
            unread_maintenance_notices: self.gateway.unread_maintenance_notices().await?,
            unjustified_non_executions: self.gateway.unjustified_non_executions().await?,
            enabled_users: self.gateway.enabled_users().await?,
            overdue_charges: if include_billing {
                Some(overdue_charges(&self.gateway.pending_charges().await?, now))
            } else {
                None
            },
        };
        log::info!(
            "[TenantDashboardUseCase] vehicles {} ({} in maintenance), open work orders {}, yard {}",
            snapshot.active_vehicles,
            snapshot.vehicles_in_maintenance,
            snapshot.open_work_orders,
            snapshot.yard_queue
        );
        Ok(snapshot)
    }
}

/// Same comparison the billing list uses: pending, and now is past
/// `period_end + grace_days`. Equality is not overdue.
pub fn overdue_charges(charges: &[PendingChargeWindow], now: NaiveDateTime) -> u64 {
    charges
        .iter()
        .filter(|charge| now > charge.period_end + Duration::days(i64::from(charge.grace_days)))
        .count() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use sea_orm::prelude::async_trait::async_trait;

    fn at(year: i32, month: u32, day: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(year, month, day)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
    }

    struct Fixed {
        charges: Vec<PendingChargeWindow>,
        billing_reads: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait]
    impl TenantDashboardStore for Fixed {
        async fn active_vehicles(&self) -> Result<u64, BusinessError> {
            Ok(4)
        }
        async fn vehicles_in_maintenance(&self) -> Result<u64, BusinessError> {
            Ok(1)
        }
        async fn open_work_orders(&self) -> Result<u64, BusinessError> {
            Ok(2)
        }
        async fn work_orders_awaiting_parts(&self) -> Result<u64, BusinessError> {
            Ok(1)
        }
        async fn yard_queue(&self) -> Result<u64, BusinessError> {
            Ok(3)
        }
        async fn unread_maintenance_notices(&self) -> Result<u64, BusinessError> {
            Ok(0)
        }
        async fn unjustified_non_executions(&self) -> Result<u64, BusinessError> {
            Ok(5)
        }
        async fn enabled_users(&self) -> Result<u64, BusinessError> {
            Ok(8)
        }
        async fn pending_charges(&self) -> Result<Vec<PendingChargeWindow>, BusinessError> {
            self.billing_reads
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(self.charges.clone())
        }
    }

    #[test]
    fn a_charge_is_overdue_only_after_its_grace_window() {
        let charges = [
            PendingChargeWindow {
                period_end: at(2026, 9, 17),
                grace_days: 0,
            },
            PendingChargeWindow {
                period_end: at(2026, 9, 17),
                grace_days: 2,
            },
            PendingChargeWindow {
                period_end: at(2026, 9, 18),
                grace_days: 0,
            },
        ];
        assert_eq!(overdue_charges(&charges, at(2026, 9, 18)), 1);
    }

    #[tokio::test]
    async fn billing_is_not_read_unless_the_caller_may_see_it() {
        let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let store = Fixed {
            charges: vec![PendingChargeWindow {
                period_end: at(2026, 9, 1),
                grace_days: 0,
            }],
            billing_reads: reads.clone(),
        };
        let snapshot = TenantDashboardUseCase::new(store)
            .snapshot(at(2026, 9, 18), false)
            .await
            .unwrap();
        assert_eq!(snapshot.active_vehicles, 4);
        assert_eq!(snapshot.overdue_charges, None);
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn the_owner_snapshot_counts_overdue_charges() {
        let store = Fixed {
            charges: vec![PendingChargeWindow {
                period_end: at(2026, 9, 1),
                grace_days: 0,
            }],
            billing_reads: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        };
        let snapshot = TenantDashboardUseCase::new(store)
            .snapshot(at(2026, 9, 18), true)
            .await
            .unwrap();
        assert_eq!(snapshot.overdue_charges, Some(1));
        assert_eq!(snapshot.unjustified_non_executions, 5);
    }
}
