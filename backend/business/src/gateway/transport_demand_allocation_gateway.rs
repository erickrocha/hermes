use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::transport_demand_allocation::{
    TransportDemandAllocation, TransportDemandAllocationEntityMapper,
};
use entity::prelude::TransportDemandAllocationEntity as AllocationQuery;
use entity::transport_demand_allocation_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

/// `HRMS-604` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct TransportDemandAllocationGateway {
    db: DbConn,
}

impl TransportDemandAllocationGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl
    Gateway<
        TransportDemandAllocation,
        transport_demand_allocation_entity::Model,
        transport_demand_allocation_entity::ActiveModel,
    > for TransportDemandAllocationGateway
{
    async fn persist(
        &self,
        entity: TransportDemandAllocation,
    ) -> Result<transport_demand_allocation_entity::ActiveModel, DbErr> {
        TransportDemandAllocationEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            AllocationQuery::delete_many().filter(transport_demand_allocation_entity::Column::Id.eq(id)),
            transport_demand_allocation_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(
        &self,
        id: i64,
    ) -> Result<Option<transport_demand_allocation_entity::Model>, DbErr> {
        tenant_select(
            AllocationQuery::find(),
            transport_demand_allocation_entity::Column::TenantId,
        )
        .filter(transport_demand_allocation_entity::Column::Id.eq(id))
        .one(&self.db)
        .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<transport_demand_allocation_entity::Model>, DbErr> {
        tenant_select(
            AllocationQuery::find(),
            transport_demand_allocation_entity::Column::TenantId,
        )
        .filter(transport_demand_allocation_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
        .one(&self.db)
        .await
    }

    async fn find_all(&self) -> Result<Vec<transport_demand_allocation_entity::Model>, DbErr> {
        tenant_select(
            AllocationQuery::find(),
            transport_demand_allocation_entity::Column::TenantId,
        )
        .all(&self.db)
        .await
    }
}

impl TransportDemandAllocationGateway {
    /// `PD-028`: a demand's allocation history, newest first.
    pub async fn find_page_by_demand(
        &self,
        demand_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<transport_demand_allocation_entity::Model>, u64), DbErr> {
        let query = tenant_select(
            AllocationQuery::find(),
            transport_demand_allocation_entity::Column::TenantId,
        )
        .filter(transport_demand_allocation_entity::Column::DemandId.eq(demand_id))
        .order_by_desc(transport_demand_allocation_entity::Column::StartDate)
        .order_by_desc(transport_demand_allocation_entity::Column::Id);
        fetch_page(query, &self.db, page, page_size).await
    }
}

/// `HRMS-604` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::transport_demand_allocation_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_allocations() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                transport_demand_allocation_entity::Entity::find(),
                transport_demand_allocation_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_allocation_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                transport_demand_allocation_entity::Entity::find(),
                transport_demand_allocation_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_an_allocation_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                transport_demand_allocation_entity::Entity::delete_many(),
                transport_demand_allocation_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
