use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::work_order_posting::{WorkOrderPosting, WorkOrderPostingEntityMapper};
use entity::prelude::WorkOrderPostingEntity as WorkOrderPostingQuery;
use entity::work_order_posting_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

/// `HRMS-705` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct WorkOrderPostingGateway {
    db: DbConn,
}

impl WorkOrderPostingGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &DbConn {
        &self.db
    }
}

#[async_trait]
impl Gateway<WorkOrderPosting, work_order_posting_entity::Model, work_order_posting_entity::ActiveModel>
    for WorkOrderPostingGateway
{
    async fn persist(
        &self,
        entity: WorkOrderPosting,
    ) -> Result<work_order_posting_entity::ActiveModel, DbErr> {
        WorkOrderPostingEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            WorkOrderPostingQuery::delete_many().filter(work_order_posting_entity::Column::Id.eq(id)),
            work_order_posting_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<work_order_posting_entity::Model>, DbErr> {
        tenant_select(WorkOrderPostingQuery::find(), work_order_posting_entity::Column::TenantId)
            .filter(work_order_posting_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<work_order_posting_entity::Model>, DbErr> {
        tenant_select(WorkOrderPostingQuery::find(), work_order_posting_entity::Column::TenantId)
            .filter(work_order_posting_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<work_order_posting_entity::Model>, DbErr> {
        tenant_select(WorkOrderPostingQuery::find(), work_order_posting_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl WorkOrderPostingGateway {
    /// `TRM-688`: the postings a work order's total cost sums over, oldest
    /// first.
    pub async fn find_by_work_order(
        &self,
        work_order_id: i64,
    ) -> Result<Vec<work_order_posting_entity::Model>, DbErr> {
        tenant_select(WorkOrderPostingQuery::find(), work_order_posting_entity::Column::TenantId)
            .filter(work_order_posting_entity::Column::WorkOrderId.eq(work_order_id))
            .order_by_asc(work_order_posting_entity::Column::Id)
            .all(&self.db)
            .await
    }
}

/// `HRMS-705` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::work_order_posting_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_postings() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                work_order_posting_entity::Entity::find(),
                work_order_posting_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_posting_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                work_order_posting_entity::Entity::find(),
                work_order_posting_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_posting_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                work_order_posting_entity::Entity::delete_many(),
                work_order_posting_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
