use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use chrono::NaiveDateTime;
use entity::garage_cycle_state_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};

#[derive(Debug, Clone, PartialEq)]
pub struct GarageCycleState {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub vehicle_id: i64,
    pub reference_odometer_meters: Option<f64>,
    pub last_observed_odometer_meters: Option<f64>,
    pub last_observed_at: Option<NaiveDateTime>,
    pub open_cycle_started_at: Option<NaiveDateTime>,
    pub open_cycle_start_odometer_meters: Option<f64>,
    pub open_cycle_distance_meters: f64,
    pub last_completed_cycle_distance_meters: Option<f64>,
    pub last_completed_cycle_at: Option<NaiveDateTime>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

impl GarageCycleState {
    pub fn new(tenant_id: Option<i64>, vehicle_id: i64) -> Self {
        Self {
            id: None,
            uuid: None,
            tenant_id,
            vehicle_id,
            reference_odometer_meters: None,
            last_observed_odometer_meters: None,
            last_observed_at: None,
            open_cycle_started_at: None,
            open_cycle_start_odometer_meters: None,
            open_cycle_distance_meters: 0.0,
            last_completed_cycle_distance_meters: None,
            last_completed_cycle_at: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }
    }

    pub fn cycle_is_open(&self) -> bool {
        self.open_cycle_started_at.is_some()
    }
}

pub struct GarageCycleStateEntityMapper;

impl EntityMapper<GarageCycleState, Model, ActiveModel> for GarageCycleStateEntityMapper {
    fn build_active_model(domain: GarageCycleState) -> ActiveModel {
        ActiveModel {
            id: domain.id.map_or(NotSet, Set),
            uuid: domain
                .uuid
                .map_or(NotSet, |uuid| Set(string_to_bytes(&uuid))),
            tenant_id: Set(domain.tenant_id),
            vehicle_id: Set(domain.vehicle_id),
            reference_odometer_meters: Set(domain.reference_odometer_meters),
            last_observed_odometer_meters: Set(domain.last_observed_odometer_meters),
            last_observed_at: Set(domain.last_observed_at.map(|at| at.and_utc())),
            open_cycle_started_at: Set(domain.open_cycle_started_at.map(|at| at.and_utc())),
            open_cycle_start_odometer_meters: Set(domain.open_cycle_start_odometer_meters),
            open_cycle_distance_meters: Set(domain.open_cycle_distance_meters),
            last_completed_cycle_distance_meters: Set(domain.last_completed_cycle_distance_meters),
            last_completed_cycle_at: Set(domain.last_completed_cycle_at.map(|at| at.and_utc())),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(model: Model) -> GarageCycleState {
        GarageCycleState {
            id: Some(model.id),
            uuid: Some(bytes_para_string(model.uuid)),
            tenant_id: model.tenant_id,
            vehicle_id: model.vehicle_id,
            reference_odometer_meters: model.reference_odometer_meters,
            last_observed_odometer_meters: model.last_observed_odometer_meters,
            last_observed_at: model.last_observed_at.map(|at| at.naive_utc()),
            open_cycle_started_at: model.open_cycle_started_at.map(|at| at.naive_utc()),
            open_cycle_start_odometer_meters: model.open_cycle_start_odometer_meters,
            open_cycle_distance_meters: model.open_cycle_distance_meters,
            last_completed_cycle_distance_meters: model.last_completed_cycle_distance_meters,
            last_completed_cycle_at: model.last_completed_cycle_at.map(|at| at.naive_utc()),
            created_at: Some(model.created_at.naive_utc()),
            created_by: model.created_by,
            updated_at: Some(model.updated_at.naive_utc()),
            updated_by: model.updated_by,
        }
    }

    fn from_active_model(mut active: ActiveModel) -> GarageCycleState {
        use sea_orm::TryIntoModel;
        match active.clone().try_into_model() {
            Ok(model) => Self::from_model(model),
            Err(_) => GarageCycleState {
                id: active.id.take(),
                uuid: active.uuid.take().map(bytes_para_string),
                tenant_id: active.tenant_id.take().flatten(),
                vehicle_id: active.vehicle_id.take().unwrap_or_default(),
                reference_odometer_meters: active.reference_odometer_meters.take().flatten(),
                last_observed_odometer_meters: active
                    .last_observed_odometer_meters
                    .take()
                    .flatten(),
                last_observed_at: active
                    .last_observed_at
                    .take()
                    .flatten()
                    .map(|at| at.naive_utc()),
                open_cycle_started_at: active
                    .open_cycle_started_at
                    .take()
                    .flatten()
                    .map(|at| at.naive_utc()),
                open_cycle_start_odometer_meters: active
                    .open_cycle_start_odometer_meters
                    .take()
                    .flatten(),
                open_cycle_distance_meters: active
                    .open_cycle_distance_meters
                    .take()
                    .unwrap_or_default(),
                last_completed_cycle_distance_meters: active
                    .last_completed_cycle_distance_meters
                    .take()
                    .flatten(),
                last_completed_cycle_at: active
                    .last_completed_cycle_at
                    .take()
                    .flatten()
                    .map(|at| at.naive_utc()),
                created_at: active.created_at.take().map(|at| at.naive_utc()),
                created_by: active.created_by.take().flatten(),
                updated_at: active.updated_at.take().map(|at| at.naive_utc()),
                updated_by: active.updated_by.take().flatten(),
            },
        }
    }
}
