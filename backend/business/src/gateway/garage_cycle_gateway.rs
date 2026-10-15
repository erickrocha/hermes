use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::tenant_select;
use crate::domain::business_error::BusinessError;
use crate::domain::garage_cycle::{GarageCycleState, GarageCycleStateEntityMapper};
use entity::garage_cycle_state_entity;
use entity::prelude::GarageCycleStateEntity as GarageCycleStateQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, DbErr, EntityTrait, QueryFilter};

#[async_trait]
pub trait GarageCycleStore: Send + Sync {
    async fn find_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<GarageCycleState>, BusinessError>;
    async fn save(&self, state: GarageCycleState) -> Result<GarageCycleState, BusinessError>;
}

/// `D-09`: one tenant-scoped garage-cycle state row per vehicle.
pub struct GarageCycleGateway {
    db: DbConn,
}

impl GarageCycleGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    async fn find_model_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<garage_cycle_state_entity::Model>, DbErr> {
        tenant_select(
            GarageCycleStateQuery::find(),
            garage_cycle_state_entity::Column::TenantId,
        )
        .filter(garage_cycle_state_entity::Column::VehicleId.eq(vehicle_id))
        .one(&self.db)
        .await
    }
}

#[async_trait]
impl GarageCycleStore for GarageCycleGateway {
    async fn find_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<GarageCycleState>, BusinessError> {
        self.find_model_by_vehicle(vehicle_id)
            .await
            .map(|model| model.map(GarageCycleStateEntityMapper::from_model))
            .map_err(database_error)
    }

    async fn save(&self, state: GarageCycleState) -> Result<GarageCycleState, BusinessError> {
        GarageCycleStateEntityMapper::build_active_model(state)
            .save(&self.db)
            .await
            .map(GarageCycleStateEntityMapper::from_active_model)
            .map_err(database_error)
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Garage cycle database error: {error}");
    log::error!("[GarageCycleGateway] {message}");
    BusinessError::new(message)
}
