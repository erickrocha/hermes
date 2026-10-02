use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::inspection_model::{InspectionModel, InspectionModelEntityMapper};
use entity::prelude::InspectionModelEntity as Query;
use entity::inspection_model_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter, QueryOrder};

/// `HRMS-715` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`.
pub struct InspectionModelGateway {
    db: DbConn,
}

impl InspectionModelGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    /// `PD-028`: every inspection model of the caller's tenant, by name.
    pub async fn find_page(&self, page: u64, page_size: u64) -> Result<(Vec<inspection_model_entity::Model>, u64), DbErr> {
        let query = tenant_select(Query::find(), inspection_model_entity::Column::TenantId)
            .order_by_asc(inspection_model_entity::Column::Name);
        fetch_page(query, &self.db, page, page_size).await
    }

    /// The tenant's model of this name (the business key).
    pub async fn find_by_name(&self, name: &str, tenant_id: Option<i64>) -> Result<Option<inspection_model_entity::Model>, DbErr> {
        tenant_select(Query::find(), inspection_model_entity::Column::TenantId)
            .filter(inspection_model_entity::Column::Name.eq(name))
            .filter(inspection_model_entity::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
    }
}

#[async_trait]
impl Gateway<InspectionModel, inspection_model_entity::Model, inspection_model_entity::ActiveModel> for InspectionModelGateway {
    async fn persist(&self, entity: InspectionModel) -> Result<inspection_model_entity::ActiveModel, DbErr> {
        InspectionModelEntityMapper::build_active_model(entity).save(&self.db).await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            Query::delete_many().filter(inspection_model_entity::Column::Id.eq(id)),
            inspection_model_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<inspection_model_entity::Model>, DbErr> {
        tenant_select(Query::find(), inspection_model_entity::Column::TenantId)
            .filter(inspection_model_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<inspection_model_entity::Model>, DbErr> {
        tenant_select(Query::find(), inspection_model_entity::Column::TenantId)
            .filter(inspection_model_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<inspection_model_entity::Model>, DbErr> {
        tenant_select(Query::find(), inspection_model_entity::Column::TenantId).all(&self.db).await
    }
}
