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
    pub unmatched: usize,
}

pub struct FuelSyncUseCase<P: FuelProvider> {
    provider: P,
    fuel_entries: FuelEntryGateway,
    vehicles: VehicleGateway,
}

impl<P: FuelProvider> FuelSyncUseCase<P> {
    pub fn new(provider: P, fuel_entries: FuelEntryGateway, vehicles: VehicleGateway) -> Self {
        Self {
            provider,
            fuel_entries,
            vehicles,
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
                    if Self::changed(&row, &tx) {
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
                    self.import(&vehicle, recorded_at, &tx).await?;
                    outcome.imported += 1;
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
            origin: FuelEntryOrigin::CtaSync,
            provider_transaction_id: Some(tx.external_id.clone()),
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        };
        self.fuel_entries.persist(corrected).await.map_err(database_error)?;
        Ok(())
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
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        };
        self.fuel_entries.persist(entry).await.map_err(database_error)?;
        Ok(())
    }
}

fn database_error(e: sea_orm::DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[FuelSyncUseCase] {}", msg);
    BusinessError::new(msg)
}
