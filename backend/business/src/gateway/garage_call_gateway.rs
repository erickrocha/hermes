use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::business_error::BusinessError;
use crate::domain::garage_call::{GarageCall, GarageCallEntityMapper};
use entity::prelude::GarageCallEntity as GarageCallQuery;
use entity::garage_call_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

#[async_trait]
pub trait GarageCallStore: Send + Sync {
    async fn save(&self, call: GarageCall) -> Result<GarageCall, BusinessError>;
    async fn get_latest_uncancelled(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<GarageCall>, BusinessError>;
}

/// `HRMS-963` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct GarageCallGateway {
    db: DbConn,
}

impl GarageCallGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl GarageCallStore for GarageCallGateway {
    async fn save(&self, call: GarageCall) -> Result<GarageCall, BusinessError> {
        let active_model = <Self as Gateway<
            GarageCall,
            garage_call_entity::Model,
            garage_call_entity::ActiveModel,
        >>::persist(self, call)
        .await
        .map_err(database_error)?;
        Ok(GarageCallEntityMapper::from_active_model(active_model))
    }

    async fn get_latest_uncancelled(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<GarageCall>, BusinessError> {
        let model = GarageCallGateway::find_latest_uncancelled(self, vehicle_id)
            .await
            .map_err(database_error)?;
        Ok(model.map(GarageCallEntityMapper::from_model))
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Garage call database error: {error}");
    log::error!("[GarageCallGateway] {message}");
    BusinessError::new(message)
}

#[async_trait]
impl Gateway<GarageCall, garage_call_entity::Model, garage_call_entity::ActiveModel>
    for GarageCallGateway
{
    async fn persist(&self, entity: GarageCall) -> Result<garage_call_entity::ActiveModel, DbErr> {
        GarageCallEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            GarageCallQuery::delete_many().filter(garage_call_entity::Column::Id.eq(id)),
            garage_call_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<garage_call_entity::Model>, DbErr> {
        tenant_select(GarageCallQuery::find(), garage_call_entity::Column::TenantId)
            .filter(garage_call_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<garage_call_entity::Model>, DbErr> {
        tenant_select(GarageCallQuery::find(), garage_call_entity::Column::TenantId)
            .filter(garage_call_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<garage_call_entity::Model>, DbErr> {
        tenant_select(GarageCallQuery::find(), garage_call_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl GarageCallGateway {
    /// `TRM-483`/`484`: the vehicle's latest call that nobody cancelled.
    pub async fn find_latest_uncancelled(&self, vehicle_id: i64) -> Result<Option<garage_call_entity::Model>, DbErr> {
        tenant_select(GarageCallQuery::find(), garage_call_entity::Column::TenantId)
            .filter(garage_call_entity::Column::VehicleId.eq(vehicle_id))
            .filter(garage_call_entity::Column::CancelledAt.is_null())
            .order_by_desc(garage_call_entity::Column::CalledAt)
            .order_by_desc(garage_call_entity::Column::Id)
            .one(&self.db)
            .await
    }
}
