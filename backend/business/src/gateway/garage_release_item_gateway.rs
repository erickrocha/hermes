use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::{tenant_delete, tenant_select};
use crate::domain::business_error::BusinessError;
use crate::domain::garage_release::{GarageReleaseItem, GarageReleaseItemEntityMapper};
use entity::garage_release_item_entity;
use sea_orm::{ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter, QueryOrder};

pub struct GarageReleaseItemGateway {
    db: DbConn,
}

impl GarageReleaseItemGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    pub async fn find_by_release(
        &self,
        release_id: i64,
    ) -> Result<Vec<GarageReleaseItem>, BusinessError> {
        tenant_select(
            garage_release_item_entity::Entity::find(),
            garage_release_item_entity::Column::TenantId,
        )
        .filter(garage_release_item_entity::Column::GarageReleaseId.eq(release_id))
        .order_by_asc(garage_release_item_entity::Column::Id)
        .all(&self.db)
        .await
        .map(GarageReleaseItemEntityMapper::from_models)
        .map_err(database_error)
    }

    pub async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, BusinessError> {
        tenant_delete(
            garage_release_item_entity::Entity::delete_many()
                .filter(garage_release_item_entity::Column::Id.eq(id)),
            garage_release_item_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
        .map_err(database_error)
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Garage release item database error: {error}");
    log::error!("[GarageReleaseItemGateway] {message}");
    BusinessError::new(message)
}
