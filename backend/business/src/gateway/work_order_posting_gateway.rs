use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::business_error::BusinessError;
use crate::domain::part::{Part, PartEntityMapper};
use crate::domain::stock_movement::{StockMovement, StockMovementEntityMapper};
use crate::domain::work_order::{WorkOrder, WorkOrderEntityMapper};
use crate::domain::work_order_item::{WorkOrderItem, WorkOrderItemEntityMapper};
use crate::domain::work_order_posting::{WorkOrderPosting, WorkOrderPostingEntityMapper};
use crate::gateway::part_gateway::PartGateway;
use crate::gateway::stock_movement_gateway::StockMovementGateway;
use crate::gateway::work_order_gateway::WorkOrderGateway;
use crate::gateway::work_order_item_gateway::WorkOrderItemGateway;
use entity::prelude::WorkOrderPostingEntity as WorkOrderPostingQuery;
use entity::work_order_posting_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder, TransactionTrait,
};

#[async_trait]
pub trait WorkOrderPostingStore: Send + Sync {
    async fn find_work_order(&self, id: i64) -> Result<Option<WorkOrder>, BusinessError>;
    async fn find_work_order_item(&self, id: i64) -> Result<Option<WorkOrderItem>, BusinessError>;
    async fn find_part(&self, id: i64) -> Result<Option<Part>, BusinessError>;
    async fn issue_atomically(
        &self,
        movement: StockMovement,
        posting: WorkOrderPosting,
    ) -> Result<WorkOrderPosting, BusinessError>;
    async fn list_by_work_order(
        &self,
        work_order_id: i64,
    ) -> Result<Vec<WorkOrderPosting>, BusinessError>;
}

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

pub struct WorkOrderPostingDatabaseGateway {
    postings: WorkOrderPostingGateway,
    work_orders: WorkOrderGateway,
    items: WorkOrderItemGateway,
    parts: PartGateway,
    movements: StockMovementGateway,
}

impl WorkOrderPostingDatabaseGateway {
    pub fn new(
        postings: WorkOrderPostingGateway,
        work_orders: WorkOrderGateway,
        items: WorkOrderItemGateway,
        parts: PartGateway,
        movements: StockMovementGateway,
    ) -> Self {
        Self {
            postings,
            work_orders,
            items,
            parts,
            movements,
        }
    }
}

#[async_trait]
impl WorkOrderPostingStore for WorkOrderPostingDatabaseGateway {
    async fn find_work_order(&self, id: i64) -> Result<Option<WorkOrder>, BusinessError> {
        <WorkOrderGateway as Gateway<
            WorkOrder,
            entity::work_order_entity::Model,
            entity::work_order_entity::ActiveModel,
        >>::find_by_id(&self.work_orders, id)
        .await
        .map(|model| model.map(WorkOrderEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn find_work_order_item(&self, id: i64) -> Result<Option<WorkOrderItem>, BusinessError> {
        <WorkOrderItemGateway as Gateway<
            WorkOrderItem,
            entity::work_order_item_entity::Model,
            entity::work_order_item_entity::ActiveModel,
        >>::find_by_id(&self.items, id)
        .await
        .map(|model| model.map(WorkOrderItemEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn find_part(&self, id: i64) -> Result<Option<Part>, BusinessError> {
        <PartGateway as Gateway<
            Part,
            entity::part_entity::Model,
            entity::part_entity::ActiveModel,
        >>::find_by_id(&self.parts, id)
        .await
        .map(|model| model.map(PartEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn issue_atomically(
        &self,
        movement: StockMovement,
        posting: WorkOrderPosting,
    ) -> Result<WorkOrderPosting, BusinessError> {
        let transaction = self.movements.db().begin().await.map_err(database_error)?;
        if let Err(error) = StockMovementEntityMapper::build_active_model(movement)
            .save(&transaction)
            .await
        {
            let _ = transaction.rollback().await;
            return Err(database_error(error));
        }
        let active = match WorkOrderPostingEntityMapper::build_active_model(posting)
            .save(&transaction)
            .await
        {
            Ok(active) => active,
            Err(error) => {
                let _ = transaction.rollback().await;
                return Err(database_error(error));
            }
        };
        transaction.commit().await.map_err(database_error)?;
        Ok(WorkOrderPostingEntityMapper::from_active_model(active))
    }

    async fn list_by_work_order(
        &self,
        work_order_id: i64,
    ) -> Result<Vec<WorkOrderPosting>, BusinessError> {
        self.postings
            .find_by_work_order(work_order_id)
            .await
            .map(WorkOrderPostingEntityMapper::from_models)
            .map_err(database_error)
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Database error: {error}");
    log::error!("[WorkOrderPostingGateway] {message}");
    BusinessError::new(message)
}

#[async_trait]
impl
    Gateway<
        WorkOrderPosting,
        work_order_posting_entity::Model,
        work_order_posting_entity::ActiveModel,
    > for WorkOrderPostingGateway
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
            WorkOrderPostingQuery::delete_many()
                .filter(work_order_posting_entity::Column::Id.eq(id)),
            work_order_posting_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<work_order_posting_entity::Model>, DbErr> {
        tenant_select(
            WorkOrderPostingQuery::find(),
            work_order_posting_entity::Column::TenantId,
        )
        .filter(work_order_posting_entity::Column::Id.eq(id))
        .one(&self.db)
        .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<work_order_posting_entity::Model>, DbErr> {
        tenant_select(
            WorkOrderPostingQuery::find(),
            work_order_posting_entity::Column::TenantId,
        )
        .filter(work_order_posting_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
        .one(&self.db)
        .await
    }

    async fn find_all(&self) -> Result<Vec<work_order_posting_entity::Model>, DbErr> {
        tenant_select(
            WorkOrderPostingQuery::find(),
            work_order_posting_entity::Column::TenantId,
        )
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
        tenant_select(
            WorkOrderPostingQuery::find(),
            work_order_posting_entity::Column::TenantId,
        )
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
