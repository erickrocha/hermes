use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::{GarageTagOrigin, VehicleStatus};
use chrono::NaiveDateTime;
use entity::vehicle_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};
use std::str::FromStr;

/// EPIC-FO-01-S01 (HRMS-920): a vehicle is a plate, a model and a status,
/// owned by exactly one tenant (HRMS-921).
///
/// `Eq` was dropped when `odometer_km: Option<f64>` (`C-023`) landed -- `f64`
/// has no total order, so it cannot implement `Eq`. `PartialEq` still holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Vehicle {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub plate: String,
    pub model: String,
    pub status: VehicleStatus,
    /// HRMS-926: the tracking provider's device reporting for this vehicle.
    pub tracker_device_id: Option<i64>,
    /// `C-023`/`HRMS-941` (operacao-trm parity, `entity-inventory.md` §1).
    pub prefix: Option<String>,
    pub vehicle_type: Option<String>,
    /// Cache of `C-025`'s kilometre-evolution record. `None` until that
    /// domain exists to write it -- no code in this crate ever sets it.
    pub odometer_km: Option<f64>,
    pub wheel_type: Option<String>,
    pub spare_tire_count: Option<i32>,
    pub spare_tire_type: Option<String>,
    pub spare_tire_notes: Option<String>,
    pub garage_tag: Option<String>,
    /// Absent, not defaulted, when no tag has ever been set (unlike
    /// `status`, which always has one). `C-028` owns writing this pair.
    pub garage_tag_origin: Option<GarageTagOrigin>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct VehicleEntityMapper {}

impl EntityMapper<Vehicle, Model, ActiveModel> for VehicleEntityMapper {
    fn build_active_model(d: Vehicle) -> ActiveModel {
        ActiveModel {
            id: match d.id {
                Some(id) => Set(id),
                None => NotSet,
            },
            uuid: match d.uuid {
                Some(uuid) => Set(string_to_bytes(&uuid)),
                None => NotSet,
            },
            // D-06: whatever is set here is overwritten by `enforce_tenant`
            // with the caller's own scope before the row reaches the database.
            tenant_id: Set(d.tenant_id),
            plate: Set(d.plate.to_owned()),
            model: Set(d.model.to_owned()),
            status: Set(d.status.to_string()),
            tracker_device_id: Set(d.tracker_device_id),
            prefix: Set(d.prefix),
            vehicle_type: Set(d.vehicle_type),
            odometer_km: Set(d.odometer_km),
            wheel_type: Set(d.wheel_type),
            spare_tire_count: Set(d.spare_tire_count),
            spare_tire_type: Set(d.spare_tire_type),
            spare_tire_notes: Set(d.spare_tire_notes),
            garage_tag: Set(d.garage_tag),
            garage_tag_origin: Set(d.garage_tag_origin.map(|o| o.to_string())),
            // Stamped by `impl_tenant_auditable_before_save!`, never by a caller.
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> Vehicle {
        Vehicle {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            plate: e.plate,
            model: e.model,
            // A stored value the vocabulary no longer knows degrades to the
            // default rather than panicking a read; writes cannot create one
            // (`VehicleUseCase` and `reject_unknown_vehicle_status` refuse it).
            status: VehicleStatus::from_str(e.status.as_str()).unwrap_or_default(),
            tracker_device_id: e.tracker_device_id,
            prefix: e.prefix,
            vehicle_type: e.vehicle_type,
            odometer_km: e.odometer_km,
            wheel_type: e.wheel_type,
            spare_tire_count: e.spare_tire_count,
            spare_tire_type: e.spare_tire_type,
            spare_tire_notes: e.spare_tire_notes,
            garage_tag: e.garage_tag,
            // A stored value the vocabulary no longer knows degrades to
            // absent, not to a guessed origin -- there is no "default" tag
            // source (see the field's own doc comment).
            garage_tag_origin: e
                .garage_tag_origin
                .and_then(|value| GarageTagOrigin::from_str(&value).ok()),
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(mut e: ActiveModel) -> Vehicle {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => Vehicle {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                tenant_id: e.tenant_id.take().flatten(),
                plate: e.plate.take().unwrap_or_default(),
                model: e.model.take().unwrap_or_default(),
                status: e
                    .status
                    .take()
                    .and_then(|value| VehicleStatus::from_str(value.as_str()).ok())
                    .unwrap_or_default(),
                tracker_device_id: e.tracker_device_id.take().flatten(),
                prefix: e.prefix.take().flatten(),
                vehicle_type: e.vehicle_type.take().flatten(),
                odometer_km: e.odometer_km.take().flatten(),
                wheel_type: e.wheel_type.take().flatten(),
                spare_tire_count: e.spare_tire_count.take().flatten(),
                spare_tire_type: e.spare_tire_type.take().flatten(),
                spare_tire_notes: e.spare_tire_notes.take().flatten(),
                garage_tag: e.garage_tag.take().flatten(),
                garage_tag_origin: e
                    .garage_tag_origin
                    .take()
                    .flatten()
                    .and_then(|value| GarageTagOrigin::from_str(&value).ok()),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
