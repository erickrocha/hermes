use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::bytes_para_string;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::internal_tank::{InternalTank, InternalTankEntityMapper};
use crate::gateway::internal_tank_gateway::InternalTankGateway;
use sea_orm::DbErr;

pub const CAPACITY_MUST_BE_POSITIVE: &str = "A tank's capacity must be positive";
pub const REFERENCE_STOCK_OUT_OF_RANGE: &str = "The reference stock must be between zero and the tank's capacity";
pub const ALERT_LEVEL_MUST_NOT_BE_NEGATIVE: &str = "The alert level cannot be negative";
pub const RESERVE_LEVEL_MUST_NOT_BE_NEGATIVE: &str = "The reserve level cannot be negative";
pub const INTERNAL_TANK_NOT_CONFIGURED: &str = "The internal tank has not been configured";

pub struct InternalTankUseCase {
    gateway: InternalTankGateway,
}

impl InternalTankUseCase {
    pub fn new(gateway: InternalTankGateway) -> Self {
        Self { gateway }
    }

    /// `TRM-1540`: "a reference stock reported by the manager and the
    /// instant that reference was taken" -- re-configuring is how the
    /// manager records a fresh physical reading, so this upserts the
    /// caller's own single row rather than appending a new one.
    pub async fn configure(&self, tank: InternalTank) -> Result<InternalTank, BusinessError> {
        let tank = Self::validated(tank)?;

        let existing = self
            .gateway
            .find_current(tank.tenant_id)
            .await
            .map_err(database_error)?;
        let tank = match existing {
            Some(model) => InternalTank {
                id: Some(model.id),
                uuid: Some(bytes_para_string(model.uuid)),
                ..tank
            },
            None => tank,
        };

        let entity = self.gateway.persist(tank).await.map_err(database_error)?;
        Ok(InternalTankEntityMapper::from_active_model(entity))
    }

    fn validated(tank: InternalTank) -> Result<InternalTank, BusinessError> {
        if tank.capacity_liters <= 0.0 {
            return Err(BusinessError::new(CAPACITY_MUST_BE_POSITIVE.to_string()));
        }
        if tank.reference_stock_liters < 0.0 || tank.reference_stock_liters > tank.capacity_liters {
            return Err(BusinessError::new(REFERENCE_STOCK_OUT_OF_RANGE.to_string()));
        }
        if tank.alert_level_liters < 0.0 {
            return Err(BusinessError::new(ALERT_LEVEL_MUST_NOT_BE_NEGATIVE.to_string()));
        }
        if tank.reserve_level_liters < 0.0 {
            return Err(BusinessError::new(RESERVE_LEVEL_MUST_NOT_BE_NEGATIVE.to_string()));
        }
        Ok(tank)
    }

    pub async fn get_current(&self, target_tenant_id: Option<i64>) -> Result<InternalTank, BusinessError> {
        let entity = self
            .gateway
            .find_current(target_tenant_id)
            .await
            .map_err(database_error)?;
        match entity {
            Some(model) => Ok(InternalTankEntityMapper::from_model(model)),
            None => Err(BusinessError::new(INTERNAL_TANK_NOT_CONFIGURED.to_string())),
        }
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[InternalTankUseCase] {}", msg);
    BusinessError::new(msg)
}
