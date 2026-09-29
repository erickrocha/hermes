use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::FuelEntryOrigin;
use chrono::NaiveDateTime;
use entity::fuel_entry_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};
use std::str::FromStr;

/// `EPIC-FU-01-S01` (`HRMS-942`): a vehicle's fuelling -- `entity-inventory.md`
/// §4 "Abastecimentos" (`TRM-514`'s base fields).
#[derive(Debug, Clone, PartialEq)]
pub struct FuelEntry {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub recorded_at: NaiveDateTime,
    pub volume_liters: f64,
    pub value_cents: i64,
    pub odometer_km: Option<f64>,
    pub station: Option<String>,
    pub full_tank: bool,
    pub origin: FuelEntryOrigin,
    pub provider_transaction_id: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct FuelEntryEntityMapper {}

impl EntityMapper<FuelEntry, Model, ActiveModel> for FuelEntryEntityMapper {
    fn build_active_model(d: FuelEntry) -> ActiveModel {
        ActiveModel {
            id: match d.id {
                Some(id) => Set(id),
                None => NotSet,
            },
            uuid: match d.uuid {
                Some(uuid) => Set(string_to_bytes(&uuid)),
                None => NotSet,
            },
            tenant_id: Set(d.tenant_id),
            vehicle_id: Set(d.vehicle_id),
            recorded_at: Set(d.recorded_at.and_utc()),
            volume_liters: Set(d.volume_liters),
            value_cents: Set(d.value_cents),
            odometer_km: Set(d.odometer_km),
            station: Set(d.station),
            full_tank: Set(d.full_tank),
            origin: Set(d.origin.to_string()),
            provider_transaction_id: Set(d.provider_transaction_id),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> FuelEntry {
        FuelEntry {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            vehicle_id: e.vehicle_id,
            recorded_at: e.recorded_at.naive_utc(),
            volume_liters: e.volume_liters,
            value_cents: e.value_cents,
            odometer_km: e.odometer_km,
            station: e.station,
            full_tank: e.full_tank,
            origin: FuelEntryOrigin::from_str(&e.origin).unwrap_or_default(),
            provider_transaction_id: e.provider_transaction_id,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> FuelEntry {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => FuelEntry {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                vehicle_id: e.vehicle_id.take().unwrap_or_default(),
                recorded_at: e
                    .recorded_at
                    .take()
                    .map(|dt| dt.naive_utc())
                    .unwrap_or_default(),
                volume_liters: e.volume_liters.take().unwrap_or_default(),
                value_cents: e.value_cents.take().unwrap_or_default(),
                odometer_km: e.odometer_km.take().flatten(),
                station: e.station.take().flatten(),
                full_tank: e.full_tank.take().unwrap_or_default(),
                origin: e
                    .origin
                    .take()
                    .and_then(|v| FuelEntryOrigin::from_str(&v).ok())
                    .unwrap_or_default(),
                provider_transaction_id: e.provider_transaction_id.take().flatten(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
