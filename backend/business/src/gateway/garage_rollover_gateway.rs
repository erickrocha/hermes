use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::garage_attendance::{GarageAttendance, GarageAttendanceEntityMapper};
use crate::domain::tenant_rule_setting::RuleSettings;
use crate::gateway::garage_attendance_gateway::GarageAttendanceGateway;
use crate::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway;
use sea_orm::prelude::async_trait::async_trait;

#[async_trait]
pub trait GarageRolloverStore: Send + Sync {
    async fn settings_for(&self, tenant_id: Option<i64>) -> Result<RuleSettings, BusinessError>;
    async fn find_all_active(&self) -> Result<Vec<GarageAttendance>, BusinessError>;
    async fn save(&self, attendance: GarageAttendance) -> Result<(), BusinessError>;
}

pub struct GarageRolloverGateway {
    attendances: GarageAttendanceGateway,
    rules: TenantRuleSettingGateway,
}

impl GarageRolloverGateway {
    pub fn new(attendances: GarageAttendanceGateway, rules: TenantRuleSettingGateway) -> Self {
        Self { attendances, rules }
    }
}

#[async_trait]
impl GarageRolloverStore for GarageRolloverGateway {
    async fn settings_for(&self, tenant_id: Option<i64>) -> Result<RuleSettings, BusinessError> {
        self.rules
            .settings_for(tenant_id)
            .await
            .map_err(database_error)
    }

    async fn find_all_active(&self) -> Result<Vec<GarageAttendance>, BusinessError> {
        self.attendances
            .find_all_active()
            .await
            .map(GarageAttendanceEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn save(&self, attendance: GarageAttendance) -> Result<(), BusinessError> {
        self.attendances
            .persist(attendance)
            .await
            .map(|_| ())
            .map_err(database_error)
    }
}

fn database_error(error: sea_orm::DbErr) -> BusinessError {
    let message = format!("Database error: {error}");
    log::error!("[GarageRolloverGateway] {message}");
    BusinessError::new(message)
}
