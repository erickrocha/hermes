use std::str::FromStr;
use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::enums::VehicleStatus;
use chrono::NaiveDateTime;
use sea_orm::{NotSet, Set};
use entity::vehicle_entity::{ActiveModel, Model};

/// EPIC-FO-01-S01 (HRMS-920): a vehicle is a plate, a model and a status,
/// owned by exactly one tenant (HRMS-921).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vehicle {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub plate: String,
    pub model: String,
    pub status: VehicleStatus,
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
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
            },
        }
    }
}
