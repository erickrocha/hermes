use business::domain::business_error::BusinessError;
use business::domain::enums::FuelEntryOrigin;
use business::domain::fuel_entry::FuelEntry;
use business::domain::internal_tank::InternalTank;
use business::gateway::fuel_entry_gateway::FuelEntryReadStore;
use business::gateway::internal_tank_gateway::InternalTankStore;
use business::use_cases::internal_tank_use_case::{InternalTankUseCase, NO_FUELLING_RECORDS};
use chrono::{NaiveDate, NaiveDateTime};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryInternalTankStore {
    tank: Mutex<Option<InternalTank>>,
}

#[async_trait]
impl InternalTankStore for MemoryInternalTankStore {
    async fn get_current(
        &self,
        target_tenant_id: Option<i64>,
    ) -> Result<Option<InternalTank>, BusinessError> {
        let tank = self
            .tank
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(tank
            .as_ref()
            .filter(|tank| target_tenant_id.is_none_or(|id| tank.tenant_id == Some(id)))
            .cloned())
    }

    async fn save(&self, mut tank: InternalTank) -> Result<InternalTank, BusinessError> {
        let mut stored = self
            .tank
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        if let Some(existing) = stored.as_ref()
            && existing.tenant_id == tank.tenant_id
        {
            tank.id = existing.id;
            tank.uuid = existing.uuid.clone();
        }
        tank.id.get_or_insert(1);
        tank.uuid.get_or_insert_with(|| "internal-tank-1".into());
        *stored = Some(tank.clone());
        Ok(tank)
    }
}

#[derive(Default)]
struct MemoryFuelLedger {
    entries: Mutex<Vec<FuelEntry>>,
}

#[async_trait]
impl FuelEntryReadStore for MemoryFuelLedger {
    async fn has_entries(&self) -> Result<bool, BusinessError> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(!entries.is_empty())
    }

    async fn reports_from(&self, from: NaiveDateTime) -> Result<Vec<FuelEntry>, BusinessError> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(entries
            .iter()
            .filter(|entry| entry.recorded_at >= from && entry.deleted_at.is_none())
            .cloned()
            .collect())
    }
}

fn at(day: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 10, day)
        .expect("valid test date")
        .and_hms_opt(12, 0, 0)
        .expect("valid test time")
}

fn tank() -> InternalTank {
    InternalTank {
        id: None,
        uuid: None,
        tenant_id: Some(42),
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
        tenant_id: Some(42),
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

#[tokio::test]
async fn internal_tank_use_case_upserts_configuration_through_its_store() {
    let use_case = InternalTankUseCase::new(
        MemoryInternalTankStore::default(),
        MemoryFuelLedger::default(),
    );

    let first = use_case
        .configure(tank())
        .await
        .expect("tank is configured");
    let mut revised = tank();
    revised.reference_stock_liters = 6_000.0;
    let updated = use_case
        .configure(revised)
        .await
        .expect("tank configuration is updated");

    assert_eq!(updated.id, first.id);
    assert_eq!(updated.uuid, first.uuid);
    assert_eq!(updated.reference_stock_liters, 6_000.0);
}

#[tokio::test]
async fn stock_is_suppressed_when_the_ledger_has_no_records() {
    let use_case = InternalTankUseCase::new(
        MemoryInternalTankStore {
            tank: Mutex::new(Some(tank())),
        },
        MemoryFuelLedger::default(),
    );

    let (_, stock) = use_case.stock(Some(42)).await.expect("stock is available");

    assert_eq!(stock.current_stock_liters, None);
    assert_eq!(stock.suppressed, Some(NO_FUELLING_RECORDS));
}

#[tokio::test]
async fn stock_counts_only_provider_fuellings_after_the_reference_time() {
    let use_case = InternalTankUseCase::new(
        MemoryInternalTankStore {
            tank: Mutex::new(Some(tank())),
        },
        MemoryFuelLedger {
            entries: Mutex::new(vec![
                fuelling(9, 300.0, FuelEntryOrigin::CtaSync),
                fuelling(11, 200.0, FuelEntryOrigin::CtaSync),
                fuelling(12, 150.0, FuelEntryOrigin::DriverPhoto),
                fuelling(13, 300.0, FuelEntryOrigin::CtaSync),
            ]),
        },
    );

    let (_, stock) = use_case.stock(Some(42)).await.expect("stock is calculated");

    assert_eq!(stock.current_stock_liters, Some(4_500.0));
    assert!(!stock.delivery_alert);
    assert_eq!(stock.suppressed, None);
}
