use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::PresenceEventKind;
use crate::domain::fuel_entry::{FuelEntry, FuelEntryEntityMapper};
use crate::domain::garage_attendance::{GarageAttendance, GarageAttendanceEntityMapper};
use crate::domain::garage_service::{GarageService, GarageServiceEntityMapper};
use crate::domain::garage_service_log::GarageServiceLog;
use crate::domain::garage_service_model::{GarageServiceModel, GarageServiceModelEntityMapper};
use crate::domain::tenant_rule_setting::RuleSettings;
use crate::gateway::fuel_entry_gateway::FuelEntryGateway;
use crate::gateway::garage_attendance_gateway::GarageAttendanceGateway;
use crate::gateway::garage_service_gateway::GarageServiceGateway;
use crate::gateway::garage_service_log_gateway::GarageServiceLogGateway;
use crate::gateway::garage_service_model_gateway::GarageServiceModelGateway;
use crate::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway;
use crate::gateway::vehicle_presence_event_gateway::VehiclePresenceEventGateway;
use chrono::NaiveDateTime;
use sea_orm::DbErr;
use sea_orm::prelude::async_trait::async_trait;

#[async_trait]
pub trait GarageFuellingMarkStore: Send + Sync {
    async fn active_attendances(&self) -> Result<Vec<GarageAttendance>, BusinessError>;
    async fn active_attendance(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<GarageAttendance>, BusinessError>;
    async fn latest_full_tank_fuel(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<FuelEntry>, BusinessError>;
    async fn services(&self, attendance_id: i64) -> Result<Vec<GarageService>, BusinessError>;
    async fn service_model(
        &self,
        model_id: i64,
    ) -> Result<Option<GarageServiceModel>, BusinessError>;
    async fn rules(&self, tenant_id: Option<i64>) -> Result<RuleSettings, BusinessError>;
    async fn last_arrival(&self, vehicle_id: i64) -> Result<Option<NaiveDateTime>, BusinessError>;
    async fn save_service(&self, service: GarageService) -> Result<GarageService, BusinessError>;
    async fn save_log(&self, log: GarageServiceLog) -> Result<(), BusinessError>;
}

pub struct GarageFuellingMarkDatabaseGateway {
    attendances: GarageAttendanceGateway,
    services: GarageServiceGateway,
    logs: GarageServiceLogGateway,
    models: GarageServiceModelGateway,
    fuel_entries: FuelEntryGateway,
    events: VehiclePresenceEventGateway,
    rules: TenantRuleSettingGateway,
}

impl GarageFuellingMarkDatabaseGateway {
    pub fn new(
        attendances: GarageAttendanceGateway,
        services: GarageServiceGateway,
        logs: GarageServiceLogGateway,
        models: GarageServiceModelGateway,
        fuel_entries: FuelEntryGateway,
        events: VehiclePresenceEventGateway,
        rules: TenantRuleSettingGateway,
    ) -> Self {
        Self {
            attendances,
            services,
            logs,
            models,
            fuel_entries,
            events,
            rules,
        }
    }
}

#[async_trait]
impl GarageFuellingMarkStore for GarageFuellingMarkDatabaseGateway {
    async fn active_attendances(&self) -> Result<Vec<GarageAttendance>, BusinessError> {
        self.attendances
            .find_all_active()
            .await
            .map(GarageAttendanceEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn active_attendance(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<GarageAttendance>, BusinessError> {
        self.attendances
            .find_active_by_vehicle(vehicle_id)
            .await
            .map(|model| model.map(GarageAttendanceEntityMapper::from_model))
            .map_err(database_error)
    }

    async fn latest_full_tank_fuel(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<FuelEntry>, BusinessError> {
        self.fuel_entries
            .find_latest_provider_full_tank(vehicle_id)
            .await
            .map(|model| model.map(FuelEntryEntityMapper::from_model))
            .map_err(database_error)
    }

    async fn services(&self, attendance_id: i64) -> Result<Vec<GarageService>, BusinessError> {
        self.services
            .find_by_attendance(attendance_id)
            .await
            .map(GarageServiceEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn service_model(
        &self,
        model_id: i64,
    ) -> Result<Option<GarageServiceModel>, BusinessError> {
        <GarageServiceModelGateway as Gateway<
            GarageServiceModel,
            entity::garage_service_model_entity::Model,
            entity::garage_service_model_entity::ActiveModel,
        >>::find_by_id(&self.models, model_id)
        .await
        .map(|model| model.map(GarageServiceModelEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn rules(&self, tenant_id: Option<i64>) -> Result<RuleSettings, BusinessError> {
        self.rules
            .settings_for(tenant_id)
            .await
            .map_err(database_error)
    }

    async fn last_arrival(&self, vehicle_id: i64) -> Result<Option<NaiveDateTime>, BusinessError> {
        self.events
            .find_latest_of_kind(vehicle_id, &PresenceEventKind::Arrival.to_string())
            .await
            .map(|event| event.map(|model| model.occurred_at.naive_utc()))
            .map_err(database_error)
    }

    async fn save_service(&self, service: GarageService) -> Result<GarageService, BusinessError> {
        self.services
            .persist(service)
            .await
            .map(GarageServiceEntityMapper::from_active_model)
            .map_err(database_error)
    }

    async fn save_log(&self, log: GarageServiceLog) -> Result<(), BusinessError> {
        self.logs
            .persist(log)
            .await
            .map(|_| ())
            .map_err(database_error)
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Database error: {error}");
    log::error!("[GarageFuellingMarkGateway] {message}");
    BusinessError::new(message)
}
