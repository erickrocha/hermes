use crate::domain::business_error::BusinessError;
use crate::domain::internal_tank::InternalTank;
use crate::domain::enums::FuelEntryOrigin;
use crate::domain::fuel_entry::FuelEntry;
use crate::gateway::fuel_entry_gateway::FuelEntryReadStore;
use crate::gateway::internal_tank_gateway::InternalTankStore;

pub const CAPACITY_MUST_BE_POSITIVE: &str = "A tank's capacity must be positive";
pub const REFERENCE_STOCK_OUT_OF_RANGE: &str = "The reference stock must be between zero and the tank's capacity";
pub const ALERT_LEVEL_MUST_NOT_BE_NEGATIVE: &str = "The alert level cannot be negative";
pub const RESERVE_LEVEL_MUST_NOT_BE_NEGATIVE: &str = "The reserve level cannot be negative";
pub const INTERNAL_TANK_NOT_CONFIGURED: &str = "The internal tank has not been configured";

pub struct InternalTankUseCase<T: InternalTankStore, F: FuelEntryReadStore> {
    gateway: T,
    fuel: F,
}

impl<T: InternalTankStore, F: FuelEntryReadStore> InternalTankUseCase<T, F> {
    pub fn new(gateway: T, fuel: F) -> Self {
        Self { gateway, fuel }
    }

    /// `TRM-1540`: "a reference stock reported by the manager and the
    /// instant that reference was taken" -- re-configuring is how the
    /// manager records a fresh physical reading, so this upserts the
    /// caller's own single row rather than appending a new one.
    pub async fn configure(&self, tank: InternalTank) -> Result<InternalTank, BusinessError> {
        let tank = Self::validated(tank)?;

        let existing = self.gateway.get_current(tank.tenant_id).await?;
        let tank = match existing {
            Some(existing) => InternalTank {
                id: existing.id,
                uuid: existing.uuid,
                ..tank
            },
            None => tank,
        };

        self.gateway.save(tank).await
    }

    fn validated(tank: InternalTank) -> Result<InternalTank, BusinessError> {
        if tank.capacity_liters <= 0.0 {
            return Err(BusinessError::new(CAPACITY_MUST_BE_POSITIVE.to_string()));
        }
        if tank.reference_stock_liters < 0.0 || tank.reference_stock_liters > tank.capacity_liters {
            return Err(BusinessError::new(REFERENCE_STOCK_OUT_OF_RANGE.to_string()));
        }
        if tank.alert_level_liters < 0.0 {
            return Err(BusinessError::new(ALERT_LEVEL_MUST_NOT_BE_NEGATIVE.to_string()));
        }
        if tank.reserve_level_liters < 0.0 {
            return Err(BusinessError::new(RESERVE_LEVEL_MUST_NOT_BE_NEGATIVE.to_string()));
        }
        Ok(tank)
    }

    pub async fn get_current(&self, target_tenant_id: Option<i64>) -> Result<InternalTank, BusinessError> {
        let tank = self.gateway.get_current(target_tenant_id).await?;
        match tank {
            Some(tank) => Ok(tank),
            None => Err(BusinessError::new(INTERNAL_TANK_NOT_CONFIGURED.to_string())),
        }
    }
}

/// `TRM-1542…1544`: the tank's current stock. `current_stock_liters` is `None`
/// (with `suppressed`) when a reference is configured but the tenant has no
/// fuelling record at all -- a full tank computed from an empty list is worse
/// than no figure (`TRM-1543`).
#[derive(Debug, Clone, PartialEq)]
pub struct TankStock {
    pub current_stock_liters: Option<f64>,
    /// `TRM-1544`: the stock has reached the alert level -- request a delivery.
    pub delivery_alert: bool,
    /// `TRM-1543`: why the figure is withheld, if it is.
    pub suppressed: Option<&'static str>,
}

pub const NO_FUELLING_RECORDS: &str =
    "No fuelling record has loaded; the stock is not shown, and the reference should not be reconfigured now";

impl<T: InternalTankStore, F: FuelEntryReadStore> InternalTankUseCase<T, F> {
    pub async fn stock(&self, target_tenant_id: Option<i64>) -> Result<(InternalTank, TankStock), BusinessError> {
        let tank = self.get_current(target_tenant_id).await?;
        if !self.fuel.has_entries().await? {
            let stock = TankStock { current_stock_liters: None, delivery_alert: false, suppressed: Some(NO_FUELLING_RECORDS) };
            return Ok((tank, stock));
        }
        let since = self.fuel.reports_from(tank.reference_at).await?;
        let current = current_stock(&tank, &since);
        let stock = TankStock {
            current_stock_liters: Some(current),
            delivery_alert: current <= tank.alert_level_liters,
            suppressed: None,
        };
        Ok((tank, stock))
    }
}

/// `TRM-1541`/`1542`: the reference stock less every fuelling posted by the
/// provider after the reference instant, floored at zero. A receipt a driver
/// photographed at an external station does not draw on this tank.
pub fn current_stock(tank: &InternalTank, entries: &[FuelEntry]) -> f64 {
    let withdrawn: f64 = entries
        .iter()
        .filter(|e| e.origin == FuelEntryOrigin::CtaSync && e.recorded_at > tank.reference_at)
        .map(|e| e.volume_liters)
        .sum();
    (tank.reference_stock_liters - withdrawn).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(day: u32) -> chrono::NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, day).unwrap().and_hms_opt(12, 0, 0).unwrap()
    }

    fn tank() -> InternalTank {
        InternalTank {
            id: None,
            uuid: None,
            tenant_id: Some(1),
            capacity_liters: 10_000.0,
            reference_stock_liters: 5_000.0,
            reference_at: at(10),
            alert_level_liters: 4_000.0,
            reserve_level_liters: 500.0,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }
    }

    fn fuelling(day: u32, liters: f64, origin: FuelEntryOrigin) -> FuelEntry {
        FuelEntry {
            id: None,
            uuid: None,
            tenant_id: Some(1),
            vehicle_id: 1,
            recorded_at: at(day),
            volume_liters: liters,
            value_cents: 0,
            odometer_km: None,
            station: None,
            full_tank: false,
            origin,
            provider_transaction_id: None,
            reported_by_user_id: None,
            odometer_override_note: None,
            provider_confirmed_at: None,
            deleted_at: None,
            unified_into_id: None,
            unification_note: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }
    }

    #[test]
    fn only_provider_fuellings_after_the_reference_draw_on_the_tank() {
        let ledger = [
            fuelling(9, 300.0, FuelEntryOrigin::CtaSync),    // before the reference
            fuelling(11, 200.0, FuelEntryOrigin::CtaSync),   // counts
            fuelling(12, 150.0, FuelEntryOrigin::DriverPhoto), // an external station: does not
            fuelling(13, 100.0, FuelEntryOrigin::Manual),    // does not
            fuelling(14, 300.0, FuelEntryOrigin::CtaSync),   // counts
        ];
        assert_eq!(current_stock(&tank(), &ledger), 4_500.0);
    }

    #[test]
    fn the_stock_never_goes_below_zero() {
        assert_eq!(current_stock(&tank(), &[fuelling(11, 9_000.0, FuelEntryOrigin::CtaSync)]), 0.0);
    }
}
