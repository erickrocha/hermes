use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::technical_inspection::{TechnicalInspection, TechnicalInspectionEntityMapper};
use entity::prelude::TechnicalInspectionEntity as Query;
use entity::technical_inspection_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter};

/// `HRMS-714` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`.
pub struct TechnicalInspectionGateway {
    db: DbConn,
}

impl TechnicalInspectionGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<TechnicalInspection, technical_inspection_entity::Model, technical_inspection_entity::ActiveModel> for TechnicalInspectionGateway {
    async fn persist(&self, entity: TechnicalInspection) -> Result<technical_inspection_entity::ActiveModel, DbErr> {
        TechnicalInspectionEntityMapper::build_active_model(entity).save(&self.db).await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            Query::delete_many().filter(technical_inspection_entity::Column::Id.eq(id)),
            technical_inspection_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<technical_inspection_entity::Model>, DbErr> {
        tenant_select(Query::find(), technical_inspection_entity::Column::TenantId)
            .filter(technical_inspection_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<technical_inspection_entity::Model>, DbErr> {
        tenant_select(Query::find(), technical_inspection_entity::Column::TenantId)
            .filter(technical_inspection_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<technical_inspection_entity::Model>, DbErr> {
        tenant_select(Query::find(), technical_inspection_entity::Column::TenantId).all(&self.db).await
    }
}
