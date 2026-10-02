use business::domain::business_error::BusinessError;
use business::domain::enums::{FuelEntryOrigin, VehicleStatus};
use business::domain::fuel_entry::FuelEntry;
use business::domain::tenant_rule_setting::TenantRuleSetting;
use business::domain::vehicle::Vehicle;
use business::gateway::fuel_entry_gateway::FuelEntryStore;
use business::gateway::tenant_rule_setting_gateway::TenantRuleSettingStore;
use business::gateway::vehicle_gateway::VehicleLookupStore;
use business::use_cases::fuel_entry_use_case::{
    FuelEntryUseCase, UNIFY_DIFFERENT_DAYS, UNIFY_NEEDS_TWO_ENTRIES, VALUE_MUST_NOT_BE_NEGATIVE,
    VEHICLE_NOT_FOUND, VOLUME_MUST_BE_POSITIVE,
};
use chrono::{NaiveDate, NaiveDateTime};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryFuelEntryStore {
    entries: Mutex<Vec<FuelEntry>>,
}

#[async_trait]
impl FuelEntryStore for MemoryFuelEntryStore {
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
        let filtered: Vec<_> = entries
            .iter()
            .filter(|entry| entry.deleted_at.is_none())
            .cloned()
            .collect();
        let total = filtered.len() as u64;
        let start = (page * page_size) as usize;
        Ok((
            filtered
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
                    && vehicle_id.is_none_or(|id| entry.vehicle_id == id)
                    && from.is_none_or(|start| entry.recorded_at >= start)
                    && to.is_none_or(|end| entry.recorded_at <= end)
                    && station
                        .as_deref()
                        .is_none_or(|value| entry.station.as_deref() == Some(value))
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
            .filter(|entry| {
                entry.vehicle_id == vehicle_id
                    && entry.deleted_at.is_some()
                    && entry.unified_into_id.is_none()
            })
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

struct MemoryRuleStore;

#[async_trait]
impl TenantRuleSettingStore for MemoryRuleStore {
    async fn get_current(
        &self,
        _target_tenant_id: Option<i64>,
    ) -> Result<Option<TenantRuleSetting>, BusinessError> {
        Ok(None)
    }

    async fn save(&self, setting: TenantRuleSetting) -> Result<TenantRuleSetting, BusinessError> {
        Ok(setting)
    }
}

fn at(day: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 10, day)
        .expect("valid test date")
        .and_hms_opt(8, 0, 0)
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
        prefix: None,
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

fn entry(day: u32, liters: f64, value_cents: i64) -> FuelEntry {
    FuelEntry {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        vehicle_id: 10,
        recorded_at: at(day),
        volume_liters: liters,
        value_cents,
        odometer_km: Some(1_000.0),
        station: Some("Station".into()),
        full_tank: false,
        origin: FuelEntryOrigin::Manual,
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

fn use_case() -> FuelEntryUseCase {
    FuelEntryUseCase::new(
        MemoryFuelEntryStore::default(),
        MemoryVehicleStore {
            vehicle: Some(vehicle()),
        },
        MemoryRuleStore,
    )
}

#[tokio::test]
async fn fuel_entry_use_case_validates_and_reports_domain_entries() {
    let use_case = use_case();
    let created = use_case
        .create(entry(1, 20.0, 3_000))
        .await
        .expect("valid entry is saved");
    let found = use_case
        .find_by_uuid(created.uuid.clone().expect("store assigned uuid"))
        .await
        .expect("entry is found");
    let (entries, liters, cents) = use_case
        .report(None, None, Some(10), None)
        .await
        .expect("report is loaded");

    assert_eq!(found.id, created.id);
    assert_eq!(entries.len(), 1);
    assert_eq!(liters, 20.0);
    assert_eq!(cents, 3_000);
}

#[tokio::test]
async fn fuel_entry_use_case_refuses_invalid_values_and_other_tenant_vehicles() {
    let use_case = use_case();
    assert_eq!(
        use_case
            .create(entry(1, 0.0, 3_000))
            .await
            .expect_err("zero volume is rejected")
            .message,
        VOLUME_MUST_BE_POSITIVE
    );
    assert_eq!(
        use_case
            .create(entry(1, 10.0, -1))
            .await
            .expect_err("negative value is rejected")
            .message,
        VALUE_MUST_NOT_BE_NEGATIVE
    );

    let other_tenant = FuelEntryUseCase::new(
        MemoryFuelEntryStore::default(),
        MemoryVehicleStore {
            vehicle: Some(Vehicle {
                tenant_id: Some(99),
                ..vehicle()
            }),
        },
        MemoryRuleStore,
    );
    assert_eq!(
        other_tenant
            .create(entry(1, 10.0, 3_000))
            .await
            .expect_err("other-tenant vehicle is hidden")
            .message,
        VEHICLE_NOT_FOUND
    );
}

#[tokio::test]
async fn unification_combines_entries_and_soft_deletes_the_source() {
    let use_case = use_case();
    let target = use_case
        .create(entry(1, 20.0, 3_000))
        .await
        .expect("target is saved");
    let source = use_case
        .create(entry(1, 10.0, 1_500))
        .await
        .expect("source is saved");

    let unified = use_case
        .unify(
            target.uuid.expect("target uuid"),
            source.uuid.expect("source uuid"),
        )
        .await
        .expect("same-day entries are unified");
    let report = use_case
        .report(None, None, Some(10), None)
        .await
        .expect("source is hidden after soft deletion");

    assert_eq!(unified.volume_liters, 30.0);
    assert_eq!(unified.value_cents, 4_500);
    assert_eq!(report.0.len(), 1);
    assert_eq!(report.0[0].id, unified.id);
    assert_eq!(report.0[0].unified_into_id, None);
}

#[tokio::test]
async fn fuel_entry_unification_rejects_different_days_and_same_entry() {
    let use_case = use_case();
    let first = use_case
        .create(entry(1, 20.0, 3_000))
        .await
        .expect("entry is saved");
    let second = use_case
        .create(entry(2, 10.0, 1_500))
        .await
        .expect("entry is saved");
    let first_uuid = first.uuid.expect("first uuid");
    let second_uuid = second.uuid.expect("second uuid");

    assert_eq!(
        use_case
            .unify(first_uuid.clone(), first_uuid.clone())
            .await
            .expect_err("one entry cannot be unified with itself")
            .message,
        UNIFY_NEEDS_TWO_ENTRIES
    );
    assert_eq!(
        use_case
            .unify(first_uuid, second_uuid)
            .await
            .expect_err("different dates cannot be unified")
            .message,
        UNIFY_DIFFERENT_DAYS
    );
}
