use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::business_error::BusinessError;
use crate::domain::garage_service_model::{GarageServiceModel, GarageServiceModelEntityMapper};
use entity::prelude::GarageServiceModelEntity as GarageServiceModelQuery;
use entity::garage_service_model_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

#[async_trait]
pub trait GarageServiceModelStore: Send + Sync {
    async fn save(&self, model: GarageServiceModel) -> Result<GarageServiceModel, BusinessError>;
    async fn get_by_id(&self, id: i64) -> Result<Option<GarageServiceModel>, BusinessError>;
    async fn get_by_uuid(&self, uuid: String) -> Result<Option<GarageServiceModel>, BusinessError>;
    async fn find_by_key(
        &self,
        name_key: &str,
        tenant_id: Option<i64>,
    ) -> Result<Option<GarageServiceModel>, BusinessError>;
    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<GarageServiceModel>, u64), BusinessError>;
}

/// `HRMS-956` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct GarageServiceModelGateway {
    db: DbConn,
}

impl GarageServiceModelGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl GarageServiceModelStore for GarageServiceModelGateway {
    async fn save(&self, model: GarageServiceModel) -> Result<GarageServiceModel, BusinessError> {
        let active_model = <Self as Gateway<
            GarageServiceModel,
            garage_service_model_entity::Model,
            garage_service_model_entity::ActiveModel,
        >>::persist(self, model)
        .await
        .map_err(database_error)?;
        Ok(GarageServiceModelEntityMapper::from_active_model(active_model))
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<GarageServiceModel>, BusinessError> {
        let model = <Self as Gateway<
            GarageServiceModel,
            garage_service_model_entity::Model,
            garage_service_model_entity::ActiveModel,
        >>::find_by_id(self, id)
        .await
        .map_err(database_error)?;
        Ok(model.map(GarageServiceModelEntityMapper::from_model))
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<GarageServiceModel>, BusinessError> {
        let model = <Self as Gateway<
            GarageServiceModel,
            garage_service_model_entity::Model,
            garage_service_model_entity::ActiveModel,
        >>::find_by_uuid(self, uuid)
        .await
        .map_err(database_error)?;
        Ok(model.map(GarageServiceModelEntityMapper::from_model))
    }

    async fn find_by_key(
        &self,
        name_key: &str,
        tenant_id: Option<i64>,
    ) -> Result<Option<GarageServiceModel>, BusinessError> {
        let model = GarageServiceModelGateway::find_by_key(self, name_key, tenant_id)
            .await
            .map_err(database_error)?;
        Ok(model.map(GarageServiceModelEntityMapper::from_model))
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<GarageServiceModel>, u64), BusinessError> {
        let (models, total) = GarageServiceModelGateway::find_page(self, page, page_size)
            .await
            .map_err(database_error)?;
        Ok((GarageServiceModelEntityMapper::from_models(models), total))
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Garage service database error: {error}");
    log::error!("[GarageServiceModelGateway] {message}");
    BusinessError::new(message)
}

#[async_trait]
impl Gateway<GarageServiceModel, garage_service_model_entity::Model, garage_service_model_entity::ActiveModel>
    for GarageServiceModelGateway
{
    async fn persist(&self, entity: GarageServiceModel) -> Result<garage_service_model_entity::ActiveModel, DbErr> {
        GarageServiceModelEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            GarageServiceModelQuery::delete_many().filter(garage_service_model_entity::Column::Id.eq(id)),
            garage_service_model_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<garage_service_model_entity::Model>, DbErr> {
        tenant_select(GarageServiceModelQuery::find(), garage_service_model_entity::Column::TenantId)
            .filter(garage_service_model_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<garage_service_model_entity::Model>, DbErr> {
        tenant_select(GarageServiceModelQuery::find(), garage_service_model_entity::Column::TenantId)
            .filter(garage_service_model_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<garage_service_model_entity::Model>, DbErr> {
        tenant_select(GarageServiceModelQuery::find(), garage_service_model_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl GarageServiceModelGateway {
    /// `PD-028`: every service of the caller's tenant, in display order.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<garage_service_model_entity::Model>, u64), DbErr> {
        let query = tenant_select(GarageServiceModelQuery::find(), garage_service_model_entity::Column::TenantId)
            .order_by_asc(garage_service_model_entity::Column::DisplayOrder)
            .order_by_asc(garage_service_model_entity::Column::Name);
        fetch_page(query, &self.db, page, page_size).await
    }

    /// The tenant's active services in display order -- what a new triage is opened against.
    pub async fn find_active(&self) -> Result<Vec<garage_service_model_entity::Model>, DbErr> {
        tenant_select(GarageServiceModelQuery::find(), garage_service_model_entity::Column::TenantId)
            .filter(garage_service_model_entity::Column::Active.eq(true))
            .order_by_asc(garage_service_model_entity::Column::DisplayOrder)
            .order_by_asc(garage_service_model_entity::Column::Name)
            .all(&self.db)
            .await
    }

    /// `TRM-433`/`434`: the tenant's service of this normalised name.
    pub async fn find_by_key(
        &self,
        name_key: &str,
        tenant_id: Option<i64>,
    ) -> Result<Option<garage_service_model_entity::Model>, DbErr> {
        tenant_select(GarageServiceModelQuery::find(), garage_service_model_entity::Column::TenantId)
            .filter(garage_service_model_entity::Column::NameKey.eq(name_key))
            .filter(garage_service_model_entity::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
    }
}
