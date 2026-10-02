use crate::domain::business_error::BusinessError;
use crate::domain::enums::{FuelEntryOrigin, Role};
use crate::domain::fuel_entry::FuelEntry;
use crate::gateway::fuel_entry_gateway::FuelEntryStore;
use crate::gateway::km_evolution_gateway::KmEvolutionStore;
use crate::gateway::tenant_rule_setting_gateway::TenantRuleSettingStore;
use crate::gateway::user_gateway::UserLookupStore;
use crate::gateway::vehicle_gateway::VehicleLookupStore;
use chrono::{Duration, NaiveDateTime};

pub const VOLUME_MUST_BE_POSITIVE: &str = "A fuelling's volume must be positive";
pub const VALUE_MUST_NOT_BE_NEGATIVE: &str = "A fuelling's value cannot be negative";
pub const NOT_A_DRIVER: &str = "The person named is not an active driver of this tenant";
pub const VEHICLE_NOT_FOUND: &str = "Vehicle not found";
pub const PREFIX_MISMATCH: &str = "The confirmed prefix does not match the vehicle chosen";
/// Followed by the reference and both window bounds.
pub const ODOMETER_OUTSIDE_WINDOW: &str = "The odometer is outside the readings around this date";

/// What a driver confirmed on screen (`TRM-521`).
pub struct FuelReceipt {
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub driver_id: i64,
    pub confirmed_prefix: String,
    pub recorded_at: NaiveDateTime,
    pub volume_liters: f64,
    pub value_cents: i64,
    pub odometer_km: Option<f64>,
    pub station: Option<String>,
    /// `TRM-537`: stated explicitly, no default.
    pub full_tank: bool,
    /// `TRM-540`: the driver's explicit confirmation of a divergent odometer.
    pub override_odometer: bool,
}

/// `EPIC-FU-03-S01` (`HRMS-948`): recording a fuelling a driver reported from
/// a receipt (`TRM-520`, `TRM-535…543`).
pub struct FuelReceiptUseCase {
    fuel: Box<dyn FuelEntryStore>,
    vehicles: Box<dyn VehicleLookupStore>,
    users: Box<dyn UserLookupStore>,
    km: Box<dyn KmEvolutionStore>,
    rules: Box<dyn TenantRuleSettingStore>,
}

impl FuelReceiptUseCase {
    pub fn new<F, V, U, K, R>(fuel: F, vehicles: V, users: U, km: K, rules: R) -> Self
    where
        F: FuelEntryStore + 'static,
        V: VehicleLookupStore + 'static,
        U: UserLookupStore + 'static,
        K: KmEvolutionStore + 'static,
        R: TenantRuleSettingStore + 'static,
    {
        Self {
            fuel: Box::new(fuel),
            vehicles: Box::new(vehicles),
            users: Box::new(users),
            km: Box::new(km),
            rules: Box::new(rules),
        }
    }

    /// Returns the entry and whether it was newly created: a resubmission of
    /// the same fuelling answers with the existing record (`TRM-541`).
    pub async fn record(&self, receipt: FuelReceipt) -> Result<(FuelEntry, bool), BusinessError> {
        if receipt.volume_liters <= 0.0 {
            return Err(BusinessError::new(VOLUME_MUST_BE_POSITIVE.to_string()));
        }
        if receipt.value_cents < 0 {
            return Err(BusinessError::new(VALUE_MUST_NOT_BE_NEGATIVE.to_string()));
        }

        // `TRM-535`: only an active driver of this tenant.
        let driver = self.users.get_by_id(receipt.driver_id).await?;
        if !driver.is_some_and(|user| {
            user.role == Role::Driver && user.enabled && user.tenant_id == receipt.tenant_id
        }) {
            return Err(BusinessError::new(NOT_A_DRIVER.to_string()));
        }

        let vehicle = self
            .vehicles
            .get_by_id(receipt.vehicle_id)
            .await?
            .filter(|v| v.tenant_id == receipt.tenant_id)
            .ok_or_else(|| BusinessError::new(VEHICLE_NOT_FOUND.to_string()))?;
        // `TRM-536`: the driver confirms the prefix of the vehicle chosen.
        if let Some(prefix) = vehicle
            .prefix
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
            && !prefix.eq_ignore_ascii_case(receipt.confirmed_prefix.trim())
        {
            return Err(BusinessError::new(PREFIX_MISMATCH.to_string()));
        }

        // `TRM-541`: the same fuelling already registered.
        let rules = self
            .rules
            .get_current(receipt.tenant_id)
            .await?
            .map(|setting| setting.settings())
            .unwrap_or_default();
        let window = Duration::minutes(rules.receipt_duplicate_window_minutes as i64);
        let nearby = self
            .fuel
            .find_report(
                Some(receipt.recorded_at - window),
                Some(receipt.recorded_at + window),
                Some(receipt.vehicle_id),
                None,
            )
            .await?;
        if let Some(existing) = nearby.into_iter().find(|entry| {
            (entry.volume_liters - receipt.volume_liters).abs()
                <= rules.receipt_duplicate_volume_tolerance_liters
        }) {
            return Ok((existing, false));
        }

        // `TRM-538…540`: judged in the receipt's own chronological place.
        let mut override_note = None;
        if let Some(km) = receipt.odometer_km {
            let before = self
                .km
                .get_before(receipt.vehicle_id, receipt.recorded_at)
                .await?;
            let after = self
                .km
                .get_after(receipt.vehicle_id, receipt.recorded_at)
                .await?;
            let (low, high) = (
                before.map(|reading| reading.km),
                after.map(|reading| reading.km),
            );
            if let Some(reference) = odometer_divergence(km, low, high) {
                let bounds = format!(
                    "reference {reference} km; window [{}, {}]",
                    show(low),
                    show(high)
                );
                if !receipt.override_odometer {
                    return Err(BusinessError::new(format!(
                        "{ODOMETER_OUTSIDE_WINDOW}: {bounds}"
                    )));
                }
                override_note = Some(format!(
                    "Odometer {km} km overridden by the driver; {bounds}"
                ));
            }
        }

        let saved = self
            .fuel
            .save(FuelEntry {
                id: None,
                uuid: None,
                tenant_id: receipt.tenant_id,
                vehicle_id: receipt.vehicle_id,
                recorded_at: receipt.recorded_at,
                volume_liters: receipt.volume_liters,
                value_cents: receipt.value_cents,
                odometer_km: receipt.odometer_km,
                station: receipt.station,
                full_tank: receipt.full_tank,
                origin: FuelEntryOrigin::DriverPhoto,
                provider_transaction_id: None,
                reported_by_user_id: Some(receipt.driver_id),
                odometer_override_note: override_note,
                provider_confirmed_at: None,
                deleted_at: None,
                unified_into_id: None,
                unification_note: None,
                created_at: None,
                created_by: None,
                updated_at: None,
                updated_by: None,
            })
            .await?;
        Ok((saved, true))
    }
}

/// `TRM-538`/`539`: the odometer must lie between the immediate neighbours
/// (either may be absent); returns the reference value when it does not.
pub fn odometer_divergence(km: f64, before: Option<f64>, after: Option<f64>) -> Option<f64> {
    match (before, after) {
        (Some(low), _) if km < low => Some(low),
        (_, Some(high)) if km > high => Some(high),
        _ => None,
    }
}

fn show(bound: Option<f64>) -> String {
    bound.map_or("-".to_string(), |v| v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_odometer_is_judged_between_its_immediate_neighbours_only() {
        assert_eq!(
            odometer_divergence(1_500.0, Some(1_000.0), Some(2_000.0)),
            None
        );
        assert_eq!(
            odometer_divergence(1_000.0, Some(1_000.0), Some(2_000.0)),
            None,
            "the bounds are inclusive"
        );
        assert_eq!(
            odometer_divergence(900.0, Some(1_000.0), Some(2_000.0)),
            Some(1_000.0)
        );
        assert_eq!(
            odometer_divergence(2_100.0, Some(1_000.0), Some(2_000.0)),
            Some(2_000.0)
        );
        // A missing neighbour is an open bound, not a violation.
        assert_eq!(odometer_divergence(50.0, None, Some(2_000.0)), None);
        assert_eq!(odometer_divergence(9_999.0, Some(1_000.0), None), None);
    }
}
