use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::service_type::{ServiceType, ServiceTypeEntityMapper};
use entity::prelude::ServiceTypeEntity as ServiceTypeQuery;
use entity::service_type_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

/// `HRMS-704` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct ServiceTypeGateway {
    db: DbConn,
}

impl ServiceTypeGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<ServiceType, service_type_entity::Model, service_type_entity::ActiveModel>
    for ServiceTypeGateway
{
    async fn persist(&self, entity: ServiceType) -> Result<service_type_entity::ActiveModel, DbErr> {
        ServiceTypeEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            ServiceTypeQuery::delete_many().filter(service_type_entity::Column::Id.eq(id)),
            service_type_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<service_type_entity::Model>, DbErr> {
        tenant_select(ServiceTypeQuery::find(), service_type_entity::Column::TenantId)
            .filter(service_type_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<service_type_entity::Model>, DbErr> {
        tenant_select(ServiceTypeQuery::find(), service_type_entity::Column::TenantId)
            .filter(service_type_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<service_type_entity::Model>, DbErr> {
        tenant_select(ServiceTypeQuery::find(), service_type_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl ServiceTypeGateway {
    /// `PD-028`: every service type of the caller's tenant, by name.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<service_type_entity::Model>, u64), DbErr> {
        let query = tenant_select(ServiceTypeQuery::find(), service_type_entity::Column::TenantId)
            .order_by_asc(service_type_entity::Column::Name);
        fetch_page(query, &self.db, page, page_size).await
    }

    /// `HRMS-704`: the pre-check `code` is already registered in this
    /// tenant, the same shape `ProvinceGateway`'s own acronym check uses.
    pub async fn find_by_code(
        &self,
        code: &str,
        tenant_id: Option<i64>,
    ) -> Result<Option<service_type_entity::Model>, DbErr> {
        tenant_select(ServiceTypeQuery::find(), service_type_entity::Column::TenantId)
            .filter(service_type_entity::Column::Code.eq(code))
            .filter(service_type_entity::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
    }
}

/// `HRMS-704` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::service_type_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_service_types() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                service_type_entity::Entity::find(),
                service_type_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_service_type_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                service_type_entity::Entity::find(),
                service_type_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_service_type_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                service_type_entity::Entity::delete_many(),
                service_type_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
