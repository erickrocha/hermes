use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::stock_movement::{StockMovement, StockMovementEntityMapper};
use entity::prelude::StockMovementEntity as StockMovementQuery;
use entity::stock_movement_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

/// `HRMS-801` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct StockMovementGateway {
    db: DbConn,
}

impl StockMovementGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &DbConn {
        &self.db
    }
}

#[async_trait]
impl Gateway<StockMovement, stock_movement_entity::Model, stock_movement_entity::ActiveModel>
    for StockMovementGateway
{
    async fn persist(
        &self,
        entity: StockMovement,
    ) -> Result<stock_movement_entity::ActiveModel, DbErr> {
        StockMovementEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            StockMovementQuery::delete_many().filter(stock_movement_entity::Column::Id.eq(id)),
            stock_movement_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<stock_movement_entity::Model>, DbErr> {
        tenant_select(StockMovementQuery::find(), stock_movement_entity::Column::TenantId)
            .filter(stock_movement_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<stock_movement_entity::Model>, DbErr> {
        tenant_select(StockMovementQuery::find(), stock_movement_entity::Column::TenantId)
            .filter(stock_movement_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<stock_movement_entity::Model>, DbErr> {
        tenant_select(StockMovementQuery::find(), stock_movement_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl StockMovementGateway {
    /// `TRM-602`: every movement of one part, oldest first -- the ledger
    /// `current_stock` sums over.
    pub async fn find_by_part(
        &self,
        part_id: i64,
    ) -> Result<Vec<stock_movement_entity::Model>, DbErr> {
        tenant_select(StockMovementQuery::find(), stock_movement_entity::Column::TenantId)
            .filter(stock_movement_entity::Column::PartId.eq(part_id))
            .order_by_asc(stock_movement_entity::Column::CreatedAt)
            .all(&self.db)
            .await
    }
}

/// `HRMS-801` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::stock_movement_entity;
    use sea_orm::{DbBackend, EntityTrait, QueryTrait};

    fn caller(tenant_id: Option<i64>, enforce_tenant: bool) -> AuditUser {
        AuditUser {
            id: 1,
            email: "owner@example.com".to_string(),
            tenant_id,
            enforce_tenant,
        }
    }

    #[tokio::test]
    async fn a_tenant_bound_caller_reads_only_its_own_stock_movements() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                stock_movement_entity::Entity::find(),
                stock_movement_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_stock_movement_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                stock_movement_entity::Entity::find(),
                stock_movement_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_stock_movement_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                stock_movement_entity::Entity::delete_many(),
                stock_movement_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
