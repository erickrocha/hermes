use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::priced_service::{PricedService, PricedServiceEntityMapper};
use entity::prelude::PricedServiceEntity as PricedServiceQuery;
use entity::priced_service_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

/// `HRMS-704` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct PricedServiceGateway {
    db: DbConn,
}

impl PricedServiceGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<PricedService, priced_service_entity::Model, priced_service_entity::ActiveModel>
    for PricedServiceGateway
{
    async fn persist(
        &self,
        entity: PricedService,
    ) -> Result<priced_service_entity::ActiveModel, DbErr> {
        PricedServiceEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            PricedServiceQuery::delete_many().filter(priced_service_entity::Column::Id.eq(id)),
            priced_service_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<priced_service_entity::Model>, DbErr> {
        tenant_select(PricedServiceQuery::find(), priced_service_entity::Column::TenantId)
            .filter(priced_service_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<priced_service_entity::Model>, DbErr> {
        tenant_select(PricedServiceQuery::find(), priced_service_entity::Column::TenantId)
            .filter(priced_service_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<priced_service_entity::Model>, DbErr> {
        tenant_select(PricedServiceQuery::find(), priced_service_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl PricedServiceGateway {
    /// `PD-028`: every priced service of the caller's tenant, by name.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<priced_service_entity::Model>, u64), DbErr> {
        let query = tenant_select(PricedServiceQuery::find(), priced_service_entity::Column::TenantId)
            .order_by_asc(priced_service_entity::Column::Name);
        fetch_page(query, &self.db, page, page_size).await
    }
}

/// `HRMS-704` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::priced_service_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_priced_services() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                priced_service_entity::Entity::find(),
                priced_service_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_priced_service_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                priced_service_entity::Entity::find(),
                priced_service_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_priced_service_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                priced_service_entity::Entity::delete_many(),
                priced_service_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
