use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::work_order_item::{WorkOrderItem, WorkOrderItemEntityMapper};
use entity::prelude::WorkOrderItemEntity as WorkOrderItemQuery;
use entity::work_order_item_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

/// `HRMS-701` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct WorkOrderItemGateway {
    db: DbConn,
}

impl WorkOrderItemGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<WorkOrderItem, work_order_item_entity::Model, work_order_item_entity::ActiveModel>
    for WorkOrderItemGateway
{
    async fn persist(
        &self,
        entity: WorkOrderItem,
    ) -> Result<work_order_item_entity::ActiveModel, DbErr> {
        WorkOrderItemEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            WorkOrderItemQuery::delete_many().filter(work_order_item_entity::Column::Id.eq(id)),
            work_order_item_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(
        &self,
        id: i64,
    ) -> Result<Option<work_order_item_entity::Model>, DbErr> {
        tenant_select(WorkOrderItemQuery::find(), work_order_item_entity::Column::TenantId)
            .filter(work_order_item_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<work_order_item_entity::Model>, DbErr> {
        tenant_select(WorkOrderItemQuery::find(), work_order_item_entity::Column::TenantId)
            .filter(work_order_item_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<work_order_item_entity::Model>, DbErr> {
        tenant_select(WorkOrderItemQuery::find(), work_order_item_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl WorkOrderItemGateway {
    /// A work order's items, in the order they were added.
    pub async fn find_by_work_order(
        &self,
        work_order_id: i64,
    ) -> Result<Vec<work_order_item_entity::Model>, DbErr> {
        tenant_select(WorkOrderItemQuery::find(), work_order_item_entity::Column::TenantId)
            .filter(work_order_item_entity::Column::WorkOrderId.eq(work_order_id))
            .order_by_asc(work_order_item_entity::Column::Id)
            .all(&self.db)
            .await
    }
}

/// `HRMS-701` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::work_order_item_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_items() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                work_order_item_entity::Entity::find(),
                work_order_item_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_item_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                work_order_item_entity::Entity::find(),
                work_order_item_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_an_item_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                work_order_item_entity::Entity::delete_many(),
                work_order_item_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
