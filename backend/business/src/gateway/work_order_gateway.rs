use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::work_order::{WorkOrder, WorkOrderEntityMapper};
use entity::prelude::WorkOrderEntity as WorkOrderQuery;
use entity::work_order_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

/// `HRMS-700` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct WorkOrderGateway {
    db: DbConn,
}

impl WorkOrderGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<WorkOrder, work_order_entity::Model, work_order_entity::ActiveModel> for WorkOrderGateway {
    async fn persist(&self, entity: WorkOrder) -> Result<work_order_entity::ActiveModel, DbErr> {
        WorkOrderEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            WorkOrderQuery::delete_many().filter(work_order_entity::Column::Id.eq(id)),
            work_order_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<work_order_entity::Model>, DbErr> {
        tenant_select(WorkOrderQuery::find(), work_order_entity::Column::TenantId)
            .filter(work_order_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<work_order_entity::Model>, DbErr> {
        tenant_select(WorkOrderQuery::find(), work_order_entity::Column::TenantId)
            .filter(work_order_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<work_order_entity::Model>, DbErr> {
        tenant_select(WorkOrderQuery::find(), work_order_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl WorkOrderGateway {
    /// `PD-028`: every work order of the caller's tenant, most recently
    /// opened first.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<work_order_entity::Model>, u64), DbErr> {
        let query = tenant_select(WorkOrderQuery::find(), work_order_entity::Column::TenantId)
            .order_by_desc(work_order_entity::Column::OpenedAt);
        fetch_page(query, &self.db, page, page_size).await
    }

    /// `EPIC-MT-03-S01`: the work orders a maintenance plan covers.
    pub async fn find_by_maintenance_plan(
        &self,
        maintenance_plan_id: i64,
    ) -> Result<Vec<work_order_entity::Model>, DbErr> {
        tenant_select(WorkOrderQuery::find(), work_order_entity::Column::TenantId)
            .filter(work_order_entity::Column::MaintenancePlanId.eq(maintenance_plan_id))
            .all(&self.db)
            .await
    }
}

/// `HRMS-700` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::work_order_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_work_orders() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                work_order_entity::Entity::find(),
                work_order_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_work_order_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                work_order_entity::Entity::find(),
                work_order_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_work_order_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                work_order_entity::Entity::delete_many(),
                work_order_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
