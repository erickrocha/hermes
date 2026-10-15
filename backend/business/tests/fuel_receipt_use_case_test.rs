use business::domain::business_error::BusinessError;
use business::domain::enums::{FuelEntryOrigin, Role, VehicleStatus};
use business::domain::fuel_entry::FuelEntry;
use business::domain::km_evolution::KmEvolution;
use business::domain::tenant_rule_setting::TenantRuleSetting;
use business::domain::user::User;
use business::domain::vehicle::Vehicle;
use business::gateway::fuel_entry_gateway::FuelEntryStore;
use business::gateway::km_evolution_gateway::KmEvolutionStore;
use business::gateway::tenant_rule_setting_gateway::TenantRuleSettingStore;
use business::gateway::user_gateway::UserLookupStore;
use business::gateway::vehicle_gateway::VehicleLookupStore;
use business::use_cases::fuel_receipt_use_case::{FuelReceipt, FuelReceiptUseCase};
use chrono::{NaiveDate, NaiveDateTime};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryFuelStore {
    entries: Mutex<Vec<FuelEntry>>,
}

#[async_trait]
impl FuelEntryStore for MemoryFuelStore {
    async fn save(&self, mut entry: FuelEntry) -> Result<FuelEntry, BusinessError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let id = entry.id.unwrap_or(entries.len() as i64 + 1);
        entry.id = Some(id);
        entry.uuid = Some(format!("fuel-entry-{id}"));
        if let Some(existing) = entries.iter_mut().find(|existing| existing.id == Some(id)) {
            *existing = entry.clone();
        } else {
            entries.push(entry.clone());
        }
        Ok(entry)
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<FuelEntry>, BusinessError> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(entries
            .iter()
            .find(|entry| {
                entry.uuid.as_deref() == Some(uuid.as_str()) && entry.deleted_at.is_none()
            })
            .cloned())
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<FuelEntry>, u64), BusinessError> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let active: Vec<_> = entries
            .iter()
            .filter(|entry| entry.deleted_at.is_none())
            .cloned()
            .collect();
        let total = active.len() as u64;
        let start = (page * page_size) as usize;
        Ok((
            active
                .into_iter()
                .skip(start)
                .take(page_size as usize)
                .collect(),
            total,
        ))
    }

    async fn find_report(
        &self,
        from: Option<NaiveDateTime>,
        to: Option<NaiveDateTime>,
        vehicle_id: Option<i64>,
        station: Option<String>,
    ) -> Result<Vec<FuelEntry>, BusinessError> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(entries
            .iter()
            .filter(|entry| {
                entry.deleted_at.is_none()
                    && from.is_none_or(|from| entry.recorded_at >= from)
                    && to.is_none_or(|to| entry.recorded_at <= to)
                    && vehicle_id.is_none_or(|id| entry.vehicle_id == id)
                    && station
                        .as_deref()
                        .is_none_or(|station| entry.station.as_deref() == Some(station))
            })
            .cloned()
            .collect())
    }

    async fn get_last_deleted_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<FuelEntry>, BusinessError> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(entries
            .iter()
            .filter(|entry| entry.vehicle_id == vehicle_id && entry.deleted_at.is_some())
            .max_by_key(|entry| (entry.deleted_at, entry.id))
            .cloned())
    }
}

struct MemoryVehicleStore {
    vehicle: Option<Vehicle>,
}

#[async_trait]
impl VehicleLookupStore for MemoryVehicleStore {
    async fn get_by_id(&self, id: i64) -> Result<Option<Vehicle>, BusinessError> {
        Ok(self
            .vehicle
            .as_ref()
            .filter(|vehicle| vehicle.id == Some(id))
            .cloned())
    }
}

struct MemoryUserStore {
    user: Option<User>,
}

#[async_trait]
impl UserLookupStore for MemoryUserStore {
    async fn get_by_id(&self, id: i64) -> Result<Option<User>, BusinessError> {
        Ok(self
            .user
            .as_ref()
            .filter(|user| user.id == Some(id))
            .cloned())
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<User>, BusinessError> {
        Ok(self
            .user
            .as_ref()
            .filter(|user| user.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }
}

#[derive(Default)]
struct MemoryKmStore;

#[async_trait]
impl KmEvolutionStore for MemoryKmStore {
    async fn save(&self, reading: KmEvolution) -> Result<KmEvolution, BusinessError> {
        Ok(reading)
    }

    async fn get_by_uuid(&self, _uuid: String) -> Result<Option<KmEvolution>, BusinessError> {
        Ok(None)
    }

    async fn get_latest_by_vehicle(
        &self,
        _vehicle_id: i64,
    ) -> Result<Option<KmEvolution>, BusinessError> {
        Ok(None)
    }

    async fn get_before(
        &self,
        _vehicle_id: i64,
        _at: NaiveDateTime,
    ) -> Result<Option<KmEvolution>, BusinessError> {
        Ok(None)
    }

    async fn get_after(
        &self,
        _vehicle_id: i64,
        _at: NaiveDateTime,
    ) -> Result<Option<KmEvolution>, BusinessError> {
        Ok(None)
    }

    async fn list_by_vehicle(
        &self,
        _vehicle_id: i64,
        _page: u64,
        _page_size: u64,
    ) -> Result<(Vec<KmEvolution>, u64), BusinessError> {
        Ok((Vec::new(), 0))
    }
}

#[derive(Default)]
struct MemoryRules;

#[async_trait]
impl TenantRuleSettingStore for MemoryRules {
    async fn get_current(
        &self,
        _tenant_id: Option<i64>,
    ) -> Result<Option<TenantRuleSetting>, BusinessError> {
        Ok(None)
    }

    async fn save(&self, setting: TenantRuleSetting) -> Result<TenantRuleSetting, BusinessError> {
        Ok(setting)
    }
}

fn at() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 10, 2)
        .expect("valid test date")
        .and_hms_opt(10, 0, 0)
        .expect("valid test time")
}

fn vehicle() -> Vehicle {
    Vehicle {
        id: Some(10),
        uuid: Some("vehicle-10".into()),
        tenant_id: Some(42),
        plate: "ABC1D23".into(),
        model: "Truck".into(),
        status: VehicleStatus::Active,
        tracker_device_id: None,
        prefix: Some("V-1".into()),
        vehicle_type: None,
        odometer_km: None,
        wheel_type: None,
        spare_tire_count: None,
        spare_tire_type: None,
        spare_tire_notes: None,
        tank_capacity_liters: None,
        reference_km_per_liter: None,
        garage_tag: None,
        garage_tag_origin: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn driver() -> User {
    User {
        id: Some(7),
        uuid: Some("driver-7".into()),
        email: "driver@example.com".into(),
        name: Some("Driver".into()),
        password: "hash".into(),
        enabled: true,
        tenant_id: Some(42),
        role: Role::Driver,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn receipt(odometer_km: Option<f64>, override_odometer: bool) -> FuelReceipt {
    FuelReceipt {
        tenant_id: Some(42),
        vehicle_id: 10,
        driver_id: 7,
        confirmed_prefix: " v-1 ".into(),
        recorded_at: at(),
        volume_liters: 30.0,
        value_cents: 5_000,
        odometer_km,
        station: Some("Station".into()),
        full_tank: true,
        override_odometer,
    }
}

fn use_case(fuel: MemoryFuelStore) -> FuelReceiptUseCase {
    FuelReceiptUseCase::new(
        fuel,
        MemoryVehicleStore {
            vehicle: Some(vehicle()),
        },
        MemoryUserStore {
            user: Some(driver()),
        },
        MemoryKmStore,
        MemoryRules,
    )
}

#[tokio::test]
async fn a_valid_receipt_is_stored_as_a_driver_photo() {
    let use_case = use_case(MemoryFuelStore::default());

    let (entry, created) = use_case
        .record(receipt(None, false))
        .await
        .expect("valid receipt is stored");

    assert!(created);
    assert_eq!(entry.origin, FuelEntryOrigin::DriverPhoto);
    assert_eq!(entry.reported_by_user_id, Some(7));
    assert_eq!(entry.volume_liters, 30.0);
}

#[tokio::test]
async fn resubmitting_a_nearby_equal_volume_receipt_returns_the_existing_entry() {
    let fuel = MemoryFuelStore::default();
    fuel.save(FuelEntry {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        vehicle_id: 10,
        recorded_at: at(),
        volume_liters: 30.2,
        value_cents: 5_000,
        odometer_km: None,
        station: Some("Station".into()),
        full_tank: true,
        origin: FuelEntryOrigin::DriverPhoto,
        provider_transaction_id: None,
        reported_by_user_id: Some(7),
        odometer_override_note: None,
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
    .expect("existing receipt is saved");
    let use_case = use_case(fuel);

    let (existing, created) = use_case
        .record(receipt(None, false))
        .await
        .expect("duplicate receipt returns existing entry");

    assert!(!created);
    assert_eq!(existing.volume_liters, 30.2);
}
