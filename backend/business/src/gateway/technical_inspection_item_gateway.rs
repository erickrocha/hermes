use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::technical_inspection_item::{TechnicalInspectionItem, TechnicalInspectionItemEntityMapper};
use entity::prelude::TechnicalInspectionItemEntity as Query;
use entity::technical_inspection_item_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter, QueryOrder};

/// `HRMS-714` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`.
pub struct TechnicalInspectionItemGateway {
    db: DbConn,
}

impl TechnicalInspectionItemGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    /// An inspection's answers, in the order they were recorded.
    pub async fn find_by_inspection(&self, inspection_id: i64) -> Result<Vec<technical_inspection_item_entity::Model>, DbErr> {
        tenant_select(Query::find(), technical_inspection_item_entity::Column::TenantId)
            .filter(technical_inspection_item_entity::Column::TechnicalInspectionId.eq(inspection_id))
            .order_by_asc(technical_inspection_item_entity::Column::Id)
            .all(&self.db)
            .await
    }
}

#[async_trait]
impl Gateway<TechnicalInspectionItem, technical_inspection_item_entity::Model, technical_inspection_item_entity::ActiveModel> for TechnicalInspectionItemGateway {
    async fn persist(&self, entity: TechnicalInspectionItem) -> Result<technical_inspection_item_entity::ActiveModel, DbErr> {
        TechnicalInspectionItemEntityMapper::build_active_model(entity).save(&self.db).await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            Query::delete_many().filter(technical_inspection_item_entity::Column::Id.eq(id)),
            technical_inspection_item_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<technical_inspection_item_entity::Model>, DbErr> {
        tenant_select(Query::find(), technical_inspection_item_entity::Column::TenantId)
            .filter(technical_inspection_item_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<technical_inspection_item_entity::Model>, DbErr> {
        tenant_select(Query::find(), technical_inspection_item_entity::Column::TenantId)
            .filter(technical_inspection_item_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<technical_inspection_item_entity::Model>, DbErr> {
        tenant_select(Query::find(), technical_inspection_item_entity::Column::TenantId).all(&self.db).await
    }
}
