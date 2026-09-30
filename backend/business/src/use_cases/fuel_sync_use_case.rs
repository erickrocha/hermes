//! `EPIC-FU-02-S01` (`HRMS-944`, `C-029`): the fuel-provider sync's own rules.
//! `gateway::fuel_provider` only knows how to ask CTA Smart and translate the
//! reply; this module decides what a transaction means -- which vehicle it
//! belongs to (`TRM-507`), whether it is a correction or a new fuelling
//! (`TRM-511`/`512`), and what gets written to `EPIC-FU-01`'s own ledger.

use crate::commons::functions::bytes_para_string;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::FuelEntryOrigin;
use crate::domain::fuel_entry::FuelEntry;
use crate::gateway::fuel_entry_gateway::FuelEntryGateway;
use crate::gateway::fuel_provider::{AckStatus, FuelProvider, ProviderFuelTransaction};
use crate::domain::tenant_rule_setting::RuleSettings;
use crate::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use crate::use_cases::vehicle_use_case::VehicleUseCase;
use entity::vehicle_entity;

/// `TRM-501`/`517`: what one sync call did, for the triggering caller to see
/// immediately -- not persisted as its own "retrievable operational state"
/// yet (`TRM-517`'s fuller ask), since that needs a table of its own.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FuelSyncOutcome {
    pub fetched: usize,
    pub imported: usize,
    pub updated: usize,
    /// `TRM-545`/`548`: driver-reported fuellings the provider's posting was
    /// matched to, rather than imported a second time.
    pub reconciled: usize,
    pub unmatched: usize,
}

pub struct FuelSyncUseCase<P: FuelProvider> {
    provider: P,
    fuel_entries: FuelEntryGateway,
    vehicles: VehicleGateway,
    rules: TenantRuleSettingGateway,
}

impl<P: FuelProvider> FuelSyncUseCase<P> {
    pub fn new(
        provider: P,
        fuel_entries: FuelEntryGateway,
        vehicles: VehicleGateway,
        rules: TenantRuleSettingGateway,
    ) -> Self {
        Self {
            provider,
            fuel_entries,
            vehicles,
            rules,
        }
    }

    /// `TRM-500…514`: one batch, matched, written and acknowledged. Runs
    /// under the caller's own tenant scope, established ambiently at the
    /// HTTP boundary (`entity::audit::run_with_user`) the same way every
    /// other authenticated endpoint's writes already are -- there is no
    /// scheduled poller here to grant a scope to, unlike `D-05`'s concern for
    /// `vehicle_tracking`'s own ingestion.
    pub async fn sync(&self, tenant_id: Option<i64>) -> Result<FuelSyncOutcome, BusinessError> {
        let transactions = self.provider.fetch_pending().await?;
        let mut outcome = FuelSyncOutcome {
            fetched: transactions.len(),
            ..Default::default()
        };
        let mut acks: Vec<(String, AckStatus)> = Vec::with_capacity(transactions.len());
        let rules = self.rules.settings_for(tenant_id).await.map_err(database_error)?;

        for tx in transactions {
            let Some(vehicle) = self.matched_vehicle(&tx, tenant_id).await? else {
                acks.push((
                    tx.external_id.clone(),
                    AckStatus::Pending(format!(
                        "Vehicle not found (frota {} / plate {})",
                        tx.fleet_prefix.as_deref().unwrap_or(""),
                        tx.plate.as_deref().unwrap_or("")
                    )),
                ));
                outcome.unmatched += 1;
                continue;
            };

            let existing = self
                .fuel_entries
                .find_by_provider_transaction_id(&tx.external_id)
                .await
                .map_err(database_error)?;

            match existing {
                Some(row) => {
                    // A deleted fuelling stays deleted (`TRM-552`); its transaction id still
                    // stops the provider's re-send from being imported again.
                    if row.deleted_at.is_none() && Self::changed(&row, &tx) {
                        self.apply_correction(row, &tx).await?;
                        outcome.updated += 1;
                    }
                }
                None => {
                    let Some(recorded_at) = tx.recorded_at else {
                        acks.push((
                            tx.external_id.clone(),
                            AckStatus::Pending("No usable fuelling datetime".to_string()),
                        ));
                        continue;
                    };
                    if self.reconcile(&vehicle, recorded_at, &tx, &rules).await? {
                        outcome.reconciled += 1;
                    } else {
                        self.import(&vehicle, recorded_at, &tx).await?;
                        outcome.imported += 1;
                    }
                }
            }
            acks.push((tx.external_id.clone(), AckStatus::Success));
        }

        self.provider.acknowledge(&acks).await?;
        Ok(outcome)
    }

    /// `TRM-507`: the provider's `FROTA` against a vehicle's `prefix` first,
    /// falling back to the normalised plate.
    async fn matched_vehicle(
        &self,
        tx: &ProviderFuelTransaction,
        tenant_id: Option<i64>,
    ) -> Result<Option<vehicle_entity::Model>, BusinessError> {
        if let Some(prefix) = tx.fleet_prefix.as_deref()
            && let Some(vehicle) = self
                .vehicles
                .find_by_prefix(prefix, tenant_id)
                .await
                .map_err(database_error)?
        {
            return Ok(Some(vehicle));
        }
        if let Some(plate) = tx.plate.as_deref() {
            let normalised = VehicleUseCase::normalise_plate(plate);
            return self
                .vehicles
                .find_by_plate(&normalised, tenant_id)
                .await
                .map_err(database_error);
        }
        Ok(None)
    }

    /// `TRM-512`: mutable fields only -- `data_hora`, `volume_litros`,
    /// `odometro_km`, `valor`, `tanque_cheio`. The vehicle a transaction
    /// belongs to is not one of them; a correction never re-matches it.
    fn changed(row: &entity::fuel_entry_entity::Model, tx: &ProviderFuelTransaction) -> bool {
        let recorded_at_changed = tx.recorded_at.is_some_and(|dt| dt.naive_utc() != row.recorded_at.naive_utc());
        recorded_at_changed
            || (tx.volume_liters - row.volume_liters).abs() > f64::EPSILON
            || tx.value_cents != row.value_cents
            || tx.odometer_km != row.odometer_km
            || tx.full_tank != row.full_tank
    }

    async fn apply_correction(
        &self,
        row: entity::fuel_entry_entity::Model,
        tx: &ProviderFuelTransaction,
    ) -> Result<(), BusinessError> {
        let corrected = FuelEntry {
            id: Some(row.id),
            uuid: Some(bytes_para_string(row.uuid)),
            tenant_id: row.tenant_id,
            vehicle_id: row.vehicle_id,
            recorded_at: tx.recorded_at.map(|dt| dt.naive_utc()).unwrap_or(row.recorded_at.naive_utc()),
            volume_liters: tx.volume_liters,
            value_cents: tx.value_cents,
            odometer_km: tx.odometer_km,
            station: tx.station.clone().or(row.station),
            full_tank: tx.full_tank,
            // A reconciled driver record keeps its origin (`TRM-548`).
            origin: row.origin.parse().unwrap_or(FuelEntryOrigin::CtaSync),
            provider_transaction_id: Some(tx.external_id.clone()),
            reported_by_user_id: row.reported_by_user_id,
            odometer_override_note: row.odometer_override_note,
            provider_confirmed_at: row.provider_confirmed_at.map(|dt| dt.naive_utc()),
            deleted_at: None,
            unified_into_id: None,
            unification_note: row.unification_note,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        };
        self.fuel_entries.persist(corrected).await.map_err(database_error)?;
        Ok(())
    }

    /// `TRM-545…549`: a provider transaction and a driver-reported fuelling
    /// are the same event when the vehicle matches, the datetime is within the
    /// window and the volume within tolerance. The driver's record is kept and
    /// the provider's official figures overlaid on it (`TRM-548`).
    async fn reconcile(
        &self,
        vehicle: &vehicle_entity::Model,
        recorded_at: chrono::DateTime<chrono::Utc>,
        tx: &ProviderFuelTransaction,
        rules: &RuleSettings,
    ) -> Result<bool, BusinessError> {
        let at = recorded_at.naive_utc();
        let window = chrono::Duration::hours(rules.reconciliation_window_hours as i64);
        let candidates = self
            .fuel_entries
            .find_unreconciled_driver_entries(vehicle.id, at - window, at + window)
            .await
            .map_err(database_error)?;
        let Some(row) = pick_candidate(at, tx.volume_liters, candidates, rules) else {
            return Ok(false);
        };
        // `TRM-549`: re-verify at write time that nobody reconciled it meanwhile.
        let fresh = self.fuel_entries.find_by_id(row.id).await.map_err(database_error)?;
        if fresh.is_none_or(|f| f.provider_transaction_id.is_some()) {
            return Ok(false);
        }
        let adopted = FuelEntry {
            id: Some(row.id),
            uuid: Some(bytes_para_string(row.uuid)),
            tenant_id: row.tenant_id,
            vehicle_id: row.vehicle_id,
            recorded_at: at,
            volume_liters: tx.volume_liters,
            value_cents: tx.value_cents,
            odometer_km: tx.odometer_km.or(row.odometer_km),
            station: row.station,
            full_tank: tx.full_tank,
            origin: FuelEntryOrigin::DriverPhoto,
            provider_transaction_id: Some(tx.external_id.clone()),
            reported_by_user_id: row.reported_by_user_id,
            odometer_override_note: row.odometer_override_note,
            provider_confirmed_at: Some(chrono::Utc::now().naive_utc()),
            deleted_at: None,
            unified_into_id: None,
            unification_note: row.unification_note,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        };
        self.fuel_entries.persist(adopted).await.map_err(database_error)?;
        Ok(true)
    }

    async fn import(
        &self,
        vehicle: &vehicle_entity::Model,
        recorded_at: chrono::DateTime<chrono::Utc>,
        tx: &ProviderFuelTransaction,
    ) -> Result<(), BusinessError> {
        let entry = FuelEntry {
            id: None,
            uuid: None,
            tenant_id: vehicle.tenant_id,
            vehicle_id: vehicle.id,
            recorded_at: recorded_at.naive_utc(),
            volume_liters: tx.volume_liters,
            value_cents: tx.value_cents,
            odometer_km: tx.odometer_km,
            station: tx.station.clone(),
            full_tank: tx.full_tank,
            origin: FuelEntryOrigin::CtaSync,
            provider_transaction_id: Some(tx.external_id.clone()),
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
        };
        self.fuel_entries.persist(entry).await.map_err(database_error)?;
        Ok(())
    }
}

/// `TRM-545`/`547`: of the candidates whose volume is within the greater of the
/// absolute and relative tolerance, the one closest in time.
pub fn pick_candidate(
    at: chrono::NaiveDateTime,
    volume_liters: f64,
    candidates: Vec<entity::fuel_entry_entity::Model>,
    rules: &RuleSettings,
) -> Option<entity::fuel_entry_entity::Model> {
    let tolerance = rules.reconciliation_volume_tolerance_liters.max(volume_liters * rules.reconciliation_volume_tolerance_ratio);
    candidates
        .into_iter()
        .filter(|c| (c.volume_liters - volume_liters).abs() <= tolerance)
        .min_by_key(|c| (c.recorded_at.naive_utc() - at).num_seconds().abs())
}

fn database_error(e: sea_orm::DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[FuelSyncUseCase] {}", msg);
    BusinessError::new(msg)
}
