use business::domain::business_error::BusinessError;
use business::domain::enums::{FuelEntryOrigin, GarageTagOrigin, VehicleStatus};
use business::domain::fuel_entry::FuelEntry;
use business::domain::fuel_gauge_setting::FuelGaugeSetting;
use business::domain::km_evolution::KmEvolution;
use business::domain::tenant_rule_setting::TenantRuleSetting;
use business::domain::vehicle::Vehicle;
use business::gateway::fuel_entry_gateway::FuelGaugeFuelStore;
use business::gateway::fuel_gauge_setting_gateway::FuelGaugeSettingStore;
use business::gateway::km_evolution_gateway::FuelGaugeOdometerStore;
use business::gateway::tenant_rule_setting_gateway::TenantRuleSettingStore;
use business::gateway::vehicle_gateway::VehicleStore;
use business::gateway::vehicle_gateway::VehicleStoreError;
use business::use_cases::fuel_gauge_use_case::{
    ConsumptionSource, FuelGaugeUseCase, GaugeUnavailable, TankGauge,
};
use chrono::{NaiveDate, NaiveDateTime};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryFuelEntries {
    entries: Vec<FuelEntry>,
}

#[async_trait]
impl FuelGaugeFuelStore for MemoryFuelEntries {
    async fn entries_for_vehicle(&self, vehicle_id: i64) -> Result<Vec<FuelEntry>, BusinessError> {
        Ok(self
            .entries
            .iter()
            .filter(|entry| entry.vehicle_id == vehicle_id)
            .cloned()
            .collect())
    }
}

struct MemoryVehicles {
    vehicles: Mutex<Vec<Vehicle>>,
}

#[async_trait]
impl VehicleStore for MemoryVehicles {
    async fn save(&self, vehicle: Vehicle) -> Result<Vehicle, VehicleStoreError> {
        let mut vehicles = self.vehicles.lock().map_err(|_| {
            VehicleStoreError::Storage(BusinessError::new("memory store poisoned".into()))
        })?;
        if let Some(existing) = vehicles
            .iter_mut()
            .find(|existing| existing.id == vehicle.id)
        {
            *existing = vehicle.clone();
        } else {
            vehicles.push(vehicle.clone());
        }
        Ok(vehicle)
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<Vehicle>, BusinessError> {
        Ok(self
            .vehicles
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?
            .iter()
            .find(|vehicle| vehicle.id == Some(id))
            .cloned())
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<Vehicle>, BusinessError> {
        Ok(self
            .vehicles
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?
            .iter()
            .find(|vehicle| vehicle.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }

    async fn find_by_plate(
        &self,
        plate: &str,
        tenant_id: Option<i64>,
    ) -> Result<Option<Vehicle>, BusinessError> {
        Ok(self
            .vehicles
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?
            .iter()
            .find(|vehicle| vehicle.plate == plate && vehicle.tenant_id == tenant_id)
            .cloned())
    }

    async fn find_by_tracker_device(
        &self,
        device_id: i64,
    ) -> Result<Option<Vehicle>, BusinessError> {
        Ok(self
            .vehicles
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?
            .iter()
            .find(|vehicle| vehicle.tracker_device_id == Some(device_id))
            .cloned())
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
        _search: Option<&str>,
    ) -> Result<(Vec<Vehicle>, u64), BusinessError> {
        let vehicles = self
            .vehicles
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let total = vehicles.len() as u64;
        let start = (page * page_size) as usize;
        Ok((
            vehicles
                .iter()
                .skip(start)
                .take(page_size as usize)
                .cloned()
                .collect(),
            total,
        ))
    }

    async fn list_all(&self) -> Result<Vec<Vehicle>, BusinessError> {
        Ok(self
            .vehicles
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?
            .clone())
    }
}

#[derive(Default)]
struct MemoryKmReadings {
    readings: Vec<KmEvolution>,
}

#[async_trait]
impl FuelGaugeOdometerStore for MemoryKmReadings {
    async fn reading_before(
        &self,
        vehicle_id: i64,
        at: NaiveDateTime,
    ) -> Result<Option<KmEvolution>, BusinessError> {
        Ok(self
            .readings
            .iter()
            .filter(|reading| {
                reading.vehicle_id == vehicle_id
                    && reading
                        .recorded_at
                        .is_some_and(|recorded_at| recorded_at <= at)
            })
            .max_by_key(|reading| (reading.recorded_at, reading.id))
            .cloned())
    }

    async fn latest_kilometers(&self, vehicle_id: i64) -> Result<Option<f64>, BusinessError> {
        Ok(self
            .readings
            .iter()
            .filter(|reading| reading.vehicle_id == vehicle_id)
            .max_by_key(|reading| (reading.recorded_at, reading.id))
            .map(|reading| reading.km))
    }
}

#[derive(Default)]
struct MemoryGaugeSettings;

#[async_trait]
impl FuelGaugeSettingStore for MemoryGaugeSettings {
    async fn get_current(
        &self,
        _target_tenant_id: Option<i64>,
    ) -> Result<Option<FuelGaugeSetting>, BusinessError> {
        Ok(None)
    }

    async fn save(&self, setting: FuelGaugeSetting) -> Result<FuelGaugeSetting, BusinessError> {
        Ok(setting)
    }
}

#[derive(Default)]
struct MemoryRuleSettings;

#[async_trait]
impl TenantRuleSettingStore for MemoryRuleSettings {
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
    NaiveDate::from_ymd_opt(2026, 9, day)
        .expect("valid test date")
        .and_hms_opt(12, 0, 0)
        .expect("valid test time")
}

fn vehicle(capacity: Option<f64>, reference: Option<f64>) -> Vehicle {
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
        tank_capacity_liters: capacity,
        reference_km_per_liter: reference,
        garage_tag: None,
        garage_tag_origin: Option::<GarageTagOrigin>::None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn full_tank_entry(recorded_at: NaiveDateTime) -> FuelEntry {
    FuelEntry {
        id: Some(1),
        uuid: Some("fuel-1".into()),
        tenant_id: Some(42),
        vehicle_id: 10,
        recorded_at,
        volume_liters: 30.0,
        value_cents: 0,
        odometer_km: None,
        station: None,
        full_tank: true,
        origin: FuelEntryOrigin::CtaSync,
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

fn use_case(fuel: MemoryFuelEntries, vehicle: Vehicle, km: MemoryKmReadings) -> FuelGaugeUseCase {
    FuelGaugeUseCase::new(
        fuel,
        MemoryVehicles {
            vehicles: Mutex::new(vec![vehicle]),
        },
        km,
        MemoryGaugeSettings,
        MemoryRuleSettings,
    )
}

#[tokio::test]
async fn a_vehicle_without_capacity_has_no_gauge() {
    let use_case = use_case(
        MemoryFuelEntries::default(),
        vehicle(None, None),
        MemoryKmReadings::default(),
    );

    assert_eq!(
        use_case.gauge(10).await.expect("gauge is read"),
        TankGauge::Unavailable(GaugeUnavailable::NoTankRegistered)
    );
}

#[tokio::test]
async fn effective_consumption_uses_the_vehicle_reference_before_peers() {
    let use_case = use_case(
        MemoryFuelEntries::default(),
        vehicle(Some(300.0), Some(8.0)),
        MemoryKmReadings::default(),
    );

    let consumption = use_case
        .effective_consumption(10)
        .await
        .expect("reference consumption is available");

    assert_eq!(consumption.km_per_liter, Some(8.0));
    assert_eq!(consumption.source, Some(ConsumptionSource::Reference));
}

#[tokio::test]
async fn gauge_uses_odometer_history_to_fill_missing_fuelling_distance() {
    let use_case = use_case(
        MemoryFuelEntries {
            entries: vec![full_tank_entry(at(20))],
        },
        vehicle(Some(300.0), Some(10.0)),
        MemoryKmReadings {
            readings: vec![
                KmEvolution {
                    id: Some(1),
                    uuid: Some("reading-before".into()),
                    tenant_id: Some(42),
                    vehicle_id: 10,
                    km: 1_000.0,
                    recorded_at: Some(at(19)),
                    origin: business::domain::enums::KmOrigin::Manual,
                    source_entity: None,
                    source_entity_id: None,
                    notes: None,
                    recorded_by_user_id: None,
                    created_at: None,
                    created_by: None,
                    updated_at: None,
                    updated_by: None,
                },
                KmEvolution {
                    id: Some(2),
                    uuid: Some("reading-current".into()),
                    tenant_id: Some(42),
                    vehicle_id: 10,
                    km: 1_200.0,
                    recorded_at: Some(at(21)),
                    origin: business::domain::enums::KmOrigin::Manual,
                    source_entity: None,
                    source_entity_id: None,
                    notes: None,
                    recorded_by_user_id: None,
                    created_at: None,
                    created_by: None,
                    updated_at: None,
                    updated_by: None,
                },
            ],
        },
    );

    let gauge = use_case.gauge(10).await.expect("gauge is calculated");
    let TankGauge::Reading {
        remaining_liters,
        estimated,
        ..
    } = gauge
    else {
        panic!("expected a calculated gauge")
    };

    assert_eq!(remaining_liters, 280.0);
    assert!(estimated);
}

#[tokio::test]
async fn missing_reference_and_peer_history_produce_no_effective_consumption() {
    let use_case = use_case(
        MemoryFuelEntries::default(),
        vehicle(Some(300.0), None),
        MemoryKmReadings::default(),
    );

    assert_eq!(
        use_case
            .effective_consumption(10)
            .await
            .expect("absence of a usable estimate is not an error"),
        business::use_cases::fuel_gauge_use_case::EffectiveConsumption {
            km_per_liter: None,
            source: None,
        }
    );
    assert_eq!(
        use_case.gauge(10).await.unwrap(),
        TankGauge::Unavailable(GaugeUnavailable::NoReferenceFuelling)
    );
}

#[tokio::test]
async fn an_unknown_vehicle_has_no_capacity_or_consumption() {
    let use_case = use_case(
        MemoryFuelEntries::default(),
        vehicle(Some(300.0), Some(8.0)),
        MemoryKmReadings::default(),
    );

    assert_eq!(
        use_case.gauge(99).await.unwrap(),
        TankGauge::Unavailable(GaugeUnavailable::NoTankRegistered)
    );
    assert_eq!(
        use_case.effective_consumption(99).await.unwrap(),
        business::use_cases::fuel_gauge_use_case::EffectiveConsumption {
            km_per_liter: None,
            source: None,
        }
    );
}
