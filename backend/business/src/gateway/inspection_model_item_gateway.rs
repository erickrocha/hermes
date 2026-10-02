use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::inspection_model_item::{InspectionModelItem, InspectionModelItemEntityMapper};
use entity::prelude::InspectionModelItemEntity as Query;
use entity::inspection_model_item_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter, QueryOrder};

/// `HRMS-715` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`.
pub struct InspectionModelItemGateway {
    db: DbConn,
}

impl InspectionModelItemGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    /// A model's items, in the order they were declared.
    pub async fn find_by_model(&self, model_id: i64) -> Result<Vec<inspection_model_item_entity::Model>, DbErr> {
        tenant_select(Query::find(), inspection_model_item_entity::Column::TenantId)
            .filter(inspection_model_item_entity::Column::InspectionModelId.eq(model_id))
            .order_by_asc(inspection_model_item_entity::Column::Id)
            .all(&self.db)
            .await
    }
}

#[async_trait]
impl Gateway<InspectionModelItem, inspection_model_item_entity::Model, inspection_model_item_entity::ActiveModel> for InspectionModelItemGateway {
    async fn persist(&self, entity: InspectionModelItem) -> Result<inspection_model_item_entity::ActiveModel, DbErr> {
        InspectionModelItemEntityMapper::build_active_model(entity).save(&self.db).await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            Query::delete_many().filter(inspection_model_item_entity::Column::Id.eq(id)),
            inspection_model_item_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<inspection_model_item_entity::Model>, DbErr> {
        tenant_select(Query::find(), inspection_model_item_entity::Column::TenantId)
            .filter(inspection_model_item_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<inspection_model_item_entity::Model>, DbErr> {
        tenant_select(Query::find(), inspection_model_item_entity::Column::TenantId)
            .filter(inspection_model_item_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<inspection_model_item_entity::Model>, DbErr> {
        tenant_select(Query::find(), inspection_model_item_entity::Column::TenantId).all(&self.db).await
    }
}
