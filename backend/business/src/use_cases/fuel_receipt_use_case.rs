use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::{FuelEntryOrigin, Role};
use crate::domain::fuel_entry::{FuelEntry, FuelEntryEntityMapper};
use crate::gateway::fuel_entry_gateway::FuelEntryGateway;
use crate::gateway::km_evolution_gateway::KmEvolutionGateway;
use crate::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway;
use crate::gateway::user_gateway::UserGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use chrono::{Duration, NaiveDateTime};
use sea_orm::DbErr;

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
    fuel: FuelEntryGateway,
    vehicles: VehicleGateway,
    users: UserGateway,
    km: KmEvolutionGateway,
    rules: TenantRuleSettingGateway,
}

impl FuelReceiptUseCase {
    pub fn new(
        fuel: FuelEntryGateway,
        vehicles: VehicleGateway,
        users: UserGateway,
        km: KmEvolutionGateway,
        rules: TenantRuleSettingGateway,
    ) -> Self {
        Self { fuel, vehicles, users, km, rules }
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
        let driver = self.users.find_by_id(receipt.driver_id).await.map_err(database_error)?;
        if !driver.is_some_and(|u| u.role == Role::Driver.to_string() && u.enabled && u.tenant_id == receipt.tenant_id) {
            return Err(BusinessError::new(NOT_A_DRIVER.to_string()));
        }

        let vehicle = self
            .vehicles
            .find_by_id(receipt.vehicle_id)
            .await
            .map_err(database_error)?
            .filter(|v| v.tenant_id == receipt.tenant_id)
            .ok_or_else(|| BusinessError::new(VEHICLE_NOT_FOUND.to_string()))?;
        // `TRM-536`: the driver confirms the prefix of the vehicle chosen.
        if let Some(prefix) = vehicle.prefix.as_deref().map(str::trim).filter(|p| !p.is_empty())
            && !prefix.eq_ignore_ascii_case(receipt.confirmed_prefix.trim())
        {
            return Err(BusinessError::new(PREFIX_MISMATCH.to_string()));
        }

        // `TRM-541`: the same fuelling already registered.
        let rules = self.rules.settings_for(receipt.tenant_id).await.map_err(database_error)?;
        let window = Duration::minutes(rules.receipt_duplicate_window_minutes as i64);
        let nearby = self
            .fuel
            .find_report(Some(receipt.recorded_at - window), Some(receipt.recorded_at + window), Some(receipt.vehicle_id), None)
            .await
            .map_err(database_error)?;
        if let Some(existing) = nearby
            .into_iter()
            .find(|e| (e.volume_liters - receipt.volume_liters).abs() <= rules.receipt_duplicate_volume_tolerance_liters)
        {
            return Ok((FuelEntryEntityMapper::from_model(existing), false));
        }

        // `TRM-538…540`: judged in the receipt's own chronological place.
        let mut override_note = None;
        if let Some(km) = receipt.odometer_km {
            let before = self.km.find_before(receipt.vehicle_id, receipt.recorded_at).await.map_err(database_error)?;
            let after = self.km.find_after(receipt.vehicle_id, receipt.recorded_at).await.map_err(database_error)?;
            let (low, high) = (before.map(|r| r.km), after.map(|r| r.km));
            if let Some(reference) = odometer_divergence(km, low, high) {
                let bounds = format!("reference {reference} km; window [{}, {}]", show(low), show(high));
                if !receipt.override_odometer {
                    return Err(BusinessError::new(format!("{ODOMETER_OUTSIDE_WINDOW}: {bounds}")));
                }
                override_note = Some(format!("Odometer {km} km overridden by the driver; {bounds}"));
            }
        }

        let saved = self
            .fuel
            .persist(FuelEntry {
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
            .await
            .map_err(database_error)?;
        Ok((FuelEntryEntityMapper::from_active_model(saved), true))
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

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[FuelReceiptUseCase] {}", msg);
    BusinessError::new(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_odometer_is_judged_between_its_immediate_neighbours_only() {
        assert_eq!(odometer_divergence(1_500.0, Some(1_000.0), Some(2_000.0)), None);
        assert_eq!(odometer_divergence(1_000.0, Some(1_000.0), Some(2_000.0)), None, "the bounds are inclusive");
        assert_eq!(odometer_divergence(900.0, Some(1_000.0), Some(2_000.0)), Some(1_000.0));
        assert_eq!(odometer_divergence(2_100.0, Some(1_000.0), Some(2_000.0)), Some(2_000.0));
        // A missing neighbour is an open bound, not a violation.
        assert_eq!(odometer_divergence(50.0, None, Some(2_000.0)), None);
        assert_eq!(odometer_divergence(9_999.0, Some(1_000.0), None), None);
    }
}
