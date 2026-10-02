use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::business_error::BusinessError;
use crate::domain::part::{Part, PartEntityMapper};
use crate::domain::purchase_order::{PurchaseOrder, PurchaseOrderEntityMapper};
use crate::gateway::part_gateway::PartGateway;
use entity::prelude::PurchaseOrderEntity as PurchaseOrderQuery;
use entity::purchase_order_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DbConn, DbErr, DeleteResult, EntityTrait,
    QueryFilter, QueryOrder,
};

#[async_trait]
pub trait PurchaseOrderStore: Send + Sync {
    async fn find_part(&self, part_id: i64) -> Result<Option<Part>, BusinessError>;
    async fn save(&self, order: PurchaseOrder) -> Result<PurchaseOrder, BusinessError>;
    async fn find_by_id(&self, id: i64) -> Result<Option<PurchaseOrder>, BusinessError>;
    async fn find_by_uuid(&self, uuid: String) -> Result<Option<PurchaseOrder>, BusinessError>;
    async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<PurchaseOrder>, u64), BusinessError>;
    async fn find_active_by_part_and_work_order(
        &self,
        part_id: i64,
        work_order_id: i64,
    ) -> Result<Vec<PurchaseOrder>, BusinessError>;
}

/// `HRMS-802` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct PurchaseOrderGateway {
    db: DbConn,
}

impl PurchaseOrderGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

pub struct PurchaseOrderDatabaseGateway {
    orders: PurchaseOrderGateway,
    parts: PartGateway,
}

impl PurchaseOrderDatabaseGateway {
    pub fn new(orders: PurchaseOrderGateway, parts: PartGateway) -> Self {
        Self { orders, parts }
    }
}

#[async_trait]
impl PurchaseOrderStore for PurchaseOrderDatabaseGateway {
    async fn find_part(&self, part_id: i64) -> Result<Option<Part>, BusinessError> {
        <PartGateway as Gateway<
            Part,
            entity::part_entity::Model,
            entity::part_entity::ActiveModel,
        >>::find_by_id(&self.parts, part_id)
        .await
        .map(|model| model.map(PartEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn save(&self, order: PurchaseOrder) -> Result<PurchaseOrder, BusinessError> {
        <PurchaseOrderGateway as Gateway<
            PurchaseOrder,
            purchase_order_entity::Model,
            purchase_order_entity::ActiveModel,
        >>::persist(&self.orders, order)
        .await
        .map(PurchaseOrderEntityMapper::from_active_model)
        .map_err(database_error)
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<PurchaseOrder>, BusinessError> {
        <PurchaseOrderGateway as Gateway<
            PurchaseOrder,
            purchase_order_entity::Model,
            purchase_order_entity::ActiveModel,
        >>::find_by_id(&self.orders, id)
        .await
        .map(|model| model.map(PurchaseOrderEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<PurchaseOrder>, BusinessError> {
        <PurchaseOrderGateway as Gateway<
            PurchaseOrder,
            purchase_order_entity::Model,
            purchase_order_entity::ActiveModel,
        >>::find_by_uuid(&self.orders, uuid)
        .await
        .map(|model| model.map(PurchaseOrderEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<PurchaseOrder>, u64), BusinessError> {
        self.orders
            .find_page(page, page_size)
            .await
            .map(|(models, total)| (PurchaseOrderEntityMapper::from_models(models), total))
            .map_err(database_error)
    }

    async fn find_active_by_part_and_work_order(
        &self,
        part_id: i64,
        work_order_id: i64,
    ) -> Result<Vec<PurchaseOrder>, BusinessError> {
        self.orders
            .find_active_by_part_and_work_order(part_id, work_order_id)
            .await
            .map(PurchaseOrderEntityMapper::from_models)
            .map_err(database_error)
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Database error: {error}");
    log::error!("[PurchaseOrderGateway] {message}");
    BusinessError::new(message)
}

#[async_trait]
impl Gateway<PurchaseOrder, purchase_order_entity::Model, purchase_order_entity::ActiveModel>
    for PurchaseOrderGateway
{
    async fn persist(
        &self,
        entity: PurchaseOrder,
    ) -> Result<purchase_order_entity::ActiveModel, DbErr> {
        PurchaseOrderEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            PurchaseOrderQuery::delete_many().filter(purchase_order_entity::Column::Id.eq(id)),
            purchase_order_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<purchase_order_entity::Model>, DbErr> {
        tenant_select(
            PurchaseOrderQuery::find(),
            purchase_order_entity::Column::TenantId,
        )
        .filter(purchase_order_entity::Column::Id.eq(id))
        .one(&self.db)
        .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<purchase_order_entity::Model>, DbErr> {
        tenant_select(
            PurchaseOrderQuery::find(),
            purchase_order_entity::Column::TenantId,
        )
        .filter(purchase_order_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
        .one(&self.db)
        .await
    }

    async fn find_all(&self) -> Result<Vec<purchase_order_entity::Model>, DbErr> {
        tenant_select(
            PurchaseOrderQuery::find(),
            purchase_order_entity::Column::TenantId,
        )
        .all(&self.db)
        .await
    }
}

impl PurchaseOrderGateway {
    /// `PD-028`: every purchase order of the caller's tenant, most recent
    /// first.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<purchase_order_entity::Model>, u64), DbErr> {
        let query = tenant_select(
            PurchaseOrderQuery::find(),
            purchase_order_entity::Column::TenantId,
        )
        .order_by_desc(purchase_order_entity::Column::CreatedAt);
        fetch_page(query, &self.db, page, page_size).await
    }

    /// `TRM-642`: the active (`Requested`/`Ordered`) purchase orders already
    /// raised for this part on this work order -- the duplicate-refusal
    /// pre-check.
    pub async fn find_active_by_part_and_work_order(
        &self,
        part_id: i64,
        work_order_id: i64,
    ) -> Result<Vec<purchase_order_entity::Model>, DbErr> {
        tenant_select(
            PurchaseOrderQuery::find(),
            purchase_order_entity::Column::TenantId,
        )
        .filter(
            Condition::all()
                .add(purchase_order_entity::Column::PartId.eq(part_id))
                .add(purchase_order_entity::Column::WorkOrderId.eq(work_order_id))
                .add(
                    Condition::any()
                        .add(purchase_order_entity::Column::Status.eq("Requested"))
                        .add(purchase_order_entity::Column::Status.eq("Ordered")),
                ),
        )
        .all(&self.db)
        .await
    }
}

/// `HRMS-802` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::purchase_order_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_purchase_orders() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                purchase_order_entity::Entity::find(),
                purchase_order_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_purchase_order_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                purchase_order_entity::Entity::find(),
                purchase_order_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_purchase_order_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                purchase_order_entity::Entity::delete_many(),
                purchase_order_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
