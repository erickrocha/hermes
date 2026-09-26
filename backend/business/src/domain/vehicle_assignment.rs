use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::{DateTime, NaiveDateTime, Utc};
use entity::vehicle_assignment_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};

/// EPIC-FO-03-S01 (HRMS-931): a driver answers for a vehicle from `started_at`
/// until `ended_at`. At most one is live per vehicle (D-23(d)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VehicleAssignment {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub driver_id: i64,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

impl VehicleAssignment {
    pub fn is_live(&self) -> bool {
        self.ended_at.is_none()
    }
}

pub struct VehicleAssignmentEntityMapper {}

impl EntityMapper<VehicleAssignment, Model, ActiveModel> for VehicleAssignmentEntityMapper {
    fn build_active_model(d: VehicleAssignment) -> ActiveModel {
        ActiveModel {
            id: match d.id {
                Some(id) => Set(id),
                None => NotSet,
            },
            uuid: match d.uuid {
                Some(uuid) => Set(string_to_bytes(&uuid)),
                None => NotSet,
            },
            // D-06: replaced by `enforce_tenant` with the caller's own scope.
            tenant_id: Set(d.tenant_id),
            vehicle_id: Set(d.vehicle_id),
            driver_id: Set(d.driver_id),
            started_at: Set(d.started_at),
            ended_at: Set(d.ended_at),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(e: Model) -> VehicleAssignment {
        VehicleAssignment {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            tenant_id: e.tenant_id,
            vehicle_id: e.vehicle_id,
            driver_id: e.driver_id,
            started_at: e.started_at,
            ended_at: e.ended_at,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
        }
    }

    fn from_active_model(e: ActiveModel) -> VehicleAssignment {
        use sea_orm::TryIntoModel;
        match e.clone().try_into_model() {
            Ok(model) => Self::from_model(model),
            Err(_) => VehicleAssignment {
                id: e.id.clone().take(),
                uuid: e.uuid.clone().take().map(bytes_para_string),
                tenant_id: e.tenant_id.clone().take().flatten(),
                vehicle_id: e.vehicle_id.clone().take().unwrap_or_default(),
                driver_id: e.driver_id.clone().take().unwrap_or_default(),
                started_at: e.started_at.clone().take().unwrap_or_default(),
                ended_at: e.ended_at.clone().take().flatten(),
                created_at: e.created_at.clone().take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.clone().take().flatten(),
                updated_at: e.updated_at.clone().take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.clone().take().flatten(),
            },
        }
    }
}
