use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::garage_attendance::{GarageAttendance, GarageAttendanceEntityMapper};
use crate::domain::garage_cycle::GarageCycleState;
use crate::domain::garage_service::{GarageService, GarageServiceEntityMapper};
use crate::domain::garage_service_model::{GarageServiceModel, GarageServiceModelEntityMapper};
use crate::domain::preventive_plan::{PreventivePlan, PreventivePlanEntityMapper};
use crate::domain::vehicle::{Vehicle, VehicleEntityMapper};
use crate::gateway::garage_attendance_gateway::GarageAttendanceGateway;
use crate::gateway::garage_cycle_gateway::{GarageCycleGateway, GarageCycleStore};
use crate::gateway::garage_service_gateway::GarageServiceGateway;
use crate::gateway::garage_service_model_gateway::GarageServiceModelGateway;
use crate::gateway::preventive_plan_gateway::PreventivePlanGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use sea_orm::DbErr;
use sea_orm::prelude::async_trait::async_trait;

#[async_trait]
pub trait GarageQueueStore: Send + Sync {
    async fn active_attendances(&self) -> Result<Vec<GarageAttendance>, BusinessError>;
    async fn all_vehicles(&self) -> Result<Vec<Vehicle>, BusinessError>;
    async fn find_vehicle(&self, vehicle_id: i64) -> Result<Option<Vehicle>, BusinessError>;
    async fn services(&self, attendance_id: i64) -> Result<Vec<GarageService>, BusinessError>;
    async fn all_service_models(&self) -> Result<Vec<GarageServiceModel>, BusinessError>;
    async fn active_service_models(&self) -> Result<Vec<GarageServiceModel>, BusinessError>;
    async fn attendance_ids_by_vehicle(&self, vehicle_id: i64) -> Result<Vec<i64>, BusinessError>;
    async fn performed_services_by_attendances(
        &self,
        attendance_ids: Vec<i64>,
    ) -> Result<Vec<GarageService>, BusinessError>;
    async fn latest_attendance(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<GarageAttendance>, BusinessError>;
    async fn preventive_plans_for_vehicle(
        &self,
        _vehicle_id: i64,
    ) -> Result<Vec<PreventivePlan>, BusinessError> {
        Ok(Vec::new())
    }
    async fn garage_cycle_state(
        &self,
        _vehicle_id: i64,
    ) -> Result<Option<GarageCycleState>, BusinessError> {
        Ok(None)
    }
}

pub struct GarageQueueDatabaseGateway {
    attendances: GarageAttendanceGateway,
    services: GarageServiceGateway,
    models: GarageServiceModelGateway,
    vehicles: VehicleGateway,
    preventive_plans: PreventivePlanGateway,
    cycles: GarageCycleGateway,
}

impl GarageQueueDatabaseGateway {
    pub fn new(
        attendances: GarageAttendanceGateway,
        services: GarageServiceGateway,
        models: GarageServiceModelGateway,
        vehicles: VehicleGateway,
        preventive_plans: PreventivePlanGateway,
        cycles: GarageCycleGateway,
    ) -> Self {
        Self {
            attendances,
            services,
            models,
            vehicles,
            preventive_plans,
            cycles,
        }
    }
}

#[async_trait]
impl GarageQueueStore for GarageQueueDatabaseGateway {
    async fn active_attendances(&self) -> Result<Vec<GarageAttendance>, BusinessError> {
        self.attendances
            .find_all_active()
            .await
            .map(GarageAttendanceEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn all_vehicles(&self) -> Result<Vec<Vehicle>, BusinessError> {
        <VehicleGateway as Gateway<
            Vehicle,
            entity::vehicle_entity::Model,
            entity::vehicle_entity::ActiveModel,
        >>::find_all(&self.vehicles)
        .await
        .map(VehicleEntityMapper::from_models)
        .map_err(database_error)
    }

    async fn find_vehicle(&self, vehicle_id: i64) -> Result<Option<Vehicle>, BusinessError> {
        <VehicleGateway as Gateway<
            Vehicle,
            entity::vehicle_entity::Model,
            entity::vehicle_entity::ActiveModel,
        >>::find_by_id(&self.vehicles, vehicle_id)
        .await
        .map(|model| model.map(VehicleEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn services(&self, attendance_id: i64) -> Result<Vec<GarageService>, BusinessError> {
        self.services
            .find_by_attendance(attendance_id)
            .await
            .map(GarageServiceEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn all_service_models(&self) -> Result<Vec<GarageServiceModel>, BusinessError> {
        <GarageServiceModelGateway as Gateway<
            GarageServiceModel,
            entity::garage_service_model_entity::Model,
            entity::garage_service_model_entity::ActiveModel,
        >>::find_all(&self.models)
        .await
        .map(GarageServiceModelEntityMapper::from_models)
        .map_err(database_error)
    }

    async fn active_service_models(&self) -> Result<Vec<GarageServiceModel>, BusinessError> {
        self.models
            .find_active()
            .await
            .map(GarageServiceModelEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn attendance_ids_by_vehicle(&self, vehicle_id: i64) -> Result<Vec<i64>, BusinessError> {
        self.attendances
            .find_ids_by_vehicle(vehicle_id)
            .await
            .map_err(database_error)
    }

    async fn performed_services_by_attendances(
        &self,
        attendance_ids: Vec<i64>,
    ) -> Result<Vec<GarageService>, BusinessError> {
        self.services
            .find_performed_by_attendances(attendance_ids)
            .await
            .map(GarageServiceEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn latest_attendance(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<GarageAttendance>, BusinessError> {
        self.attendances
            .find_latest_by_vehicle(vehicle_id)
            .await
            .map(|model| model.map(GarageAttendanceEntityMapper::from_model))
            .map_err(database_error)
    }

    async fn preventive_plans_for_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Vec<PreventivePlan>, BusinessError> {
        self.preventive_plans
            .find_by_vehicle(vehicle_id)
            .await
            .map(PreventivePlanEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn garage_cycle_state(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<GarageCycleState>, BusinessError> {
        self.cycles.find_by_vehicle(vehicle_id).await
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Database error: {error}");
    log::error!("[GarageQueueGateway] {message}");
    BusinessError::new(message)
}
