use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::garage_attendance::{GarageAttendance, GarageAttendanceEntityMapper};
use crate::domain::garage_service::{GarageService, GarageServiceEntityMapper};
use crate::domain::garage_service_log::GarageServiceLog;
use crate::domain::garage_service_model::{GarageServiceModel, GarageServiceModelEntityMapper};
use crate::domain::vehicle::Vehicle;
use crate::gateway::garage_attendance_gateway::GarageAttendanceGateway;
use crate::gateway::garage_service_gateway::GarageServiceGateway;
use crate::gateway::garage_service_log_gateway::GarageServiceLogGateway;
use crate::gateway::garage_service_model_gateway::GarageServiceModelGateway;
use crate::gateway::vehicle_gateway::{VehicleGateway, VehicleLookupStore};
use sea_orm::DbErr;
use sea_orm::prelude::async_trait::async_trait;

#[async_trait]
pub trait GarageAttendanceStore: Send + Sync {
    async fn find_vehicle(&self, id: i64) -> Result<Option<Vehicle>, BusinessError>;
    async fn find_active_attendance(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<GarageAttendance>, BusinessError>;
    async fn find_latest_attendance(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<GarageAttendance>, BusinessError>;
    async fn active_service_models(&self) -> Result<Vec<GarageServiceModel>, BusinessError>;
    async fn save_attendance(
        &self,
        attendance: GarageAttendance,
    ) -> Result<GarageAttendance, BusinessError>;
    async fn find_attendance_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<GarageAttendance>, BusinessError>;
    async fn list_services(&self, attendance_id: i64) -> Result<Vec<GarageService>, BusinessError>;
    async fn find_service_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<GarageService>, BusinessError>;
    async fn save_service(&self, service: GarageService) -> Result<GarageService, BusinessError>;
    async fn save_service_log(&self, log: GarageServiceLog) -> Result<(), BusinessError>;
}

pub struct GarageAttendanceDatabaseGateway {
    attendances: GarageAttendanceGateway,
    services: GarageServiceGateway,
    logs: GarageServiceLogGateway,
    models: GarageServiceModelGateway,
    vehicles: VehicleGateway,
}

impl GarageAttendanceDatabaseGateway {
    pub fn new(
        attendances: GarageAttendanceGateway,
        services: GarageServiceGateway,
        logs: GarageServiceLogGateway,
        models: GarageServiceModelGateway,
        vehicles: VehicleGateway,
    ) -> Self {
        Self {
            attendances,
            services,
            logs,
            models,
            vehicles,
        }
    }
}

#[async_trait]
impl GarageAttendanceStore for GarageAttendanceDatabaseGateway {
    async fn find_vehicle(&self, id: i64) -> Result<Option<Vehicle>, BusinessError> {
        self.vehicles.get_by_id(id).await
    }

    async fn find_active_attendance(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<GarageAttendance>, BusinessError> {
        self.attendances
            .find_active_by_vehicle(vehicle_id)
            .await
            .map(|model| model.map(GarageAttendanceEntityMapper::from_model))
            .map_err(database_error)
    }

    async fn find_latest_attendance(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<GarageAttendance>, BusinessError> {
        self.attendances
            .find_latest_by_vehicle(vehicle_id)
            .await
            .map(|model| model.map(GarageAttendanceEntityMapper::from_model))
            .map_err(database_error)
    }

    async fn active_service_models(&self) -> Result<Vec<GarageServiceModel>, BusinessError> {
        self.models
            .find_active()
            .await
            .map(GarageServiceModelEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn save_attendance(
        &self,
        attendance: GarageAttendance,
    ) -> Result<GarageAttendance, BusinessError> {
        <GarageAttendanceGateway as Gateway<
            GarageAttendance,
            entity::garage_attendance_entity::Model,
            entity::garage_attendance_entity::ActiveModel,
        >>::persist(&self.attendances, attendance)
        .await
        .map(GarageAttendanceEntityMapper::from_active_model)
        .map_err(|error| {
            if error
                .to_string()
                .contains("uq_garage_attendance_active_vehicle")
            {
                BusinessError::new("This vehicle already has an active triage".to_string())
            } else {
                database_error(error)
            }
        })
    }

    async fn find_attendance_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<GarageAttendance>, BusinessError> {
        <GarageAttendanceGateway as Gateway<
            GarageAttendance,
            entity::garage_attendance_entity::Model,
            entity::garage_attendance_entity::ActiveModel,
        >>::find_by_uuid(&self.attendances, uuid)
        .await
        .map(|model| model.map(GarageAttendanceEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn list_services(&self, attendance_id: i64) -> Result<Vec<GarageService>, BusinessError> {
        self.services
            .find_by_attendance(attendance_id)
            .await
            .map(GarageServiceEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn find_service_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<GarageService>, BusinessError> {
        <GarageServiceGateway as Gateway<
            GarageService,
            entity::garage_service_entity::Model,
            entity::garage_service_entity::ActiveModel,
        >>::find_by_uuid(&self.services, uuid)
        .await
        .map(|model| model.map(GarageServiceEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn save_service(&self, service: GarageService) -> Result<GarageService, BusinessError> {
        <GarageServiceGateway as Gateway<
            GarageService,
            entity::garage_service_entity::Model,
            entity::garage_service_entity::ActiveModel,
        >>::persist(&self.services, service)
        .await
        .map(GarageServiceEntityMapper::from_active_model)
        .map_err(database_error)
    }

    async fn save_service_log(&self, log: GarageServiceLog) -> Result<(), BusinessError> {
        <GarageServiceLogGateway as Gateway<
            GarageServiceLog,
            entity::garage_service_log_entity::Model,
            entity::garage_service_log_entity::ActiveModel,
        >>::persist(&self.logs, log)
        .await
        .map(|_| ())
        .map_err(database_error)
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Database error: {error}");
    log::error!("[GarageAttendanceGateway] {message}");
    BusinessError::new(message)
}
