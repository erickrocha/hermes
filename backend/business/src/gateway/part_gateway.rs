use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::part::{Part, PartEntityMapper};
use entity::part_entity;
use entity::prelude::PartEntity as PartQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

/// `HRMS-800` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct PartGateway {
    db: DbConn,
}

impl PartGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &DbConn {
        &self.db
    }
}

#[async_trait]
impl Gateway<Part, part_entity::Model, part_entity::ActiveModel> for PartGateway {
    async fn persist(&self, entity: Part) -> Result<part_entity::ActiveModel, DbErr> {
        PartEntityMapper::build_active_model(entity).save(&self.db).await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            PartQuery::delete_many().filter(part_entity::Column::Id.eq(id)),
            part_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<part_entity::Model>, DbErr> {
        tenant_select(PartQuery::find(), part_entity::Column::TenantId)
            .filter(part_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<part_entity::Model>, DbErr> {
        tenant_select(PartQuery::find(), part_entity::Column::TenantId)
            .filter(part_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<part_entity::Model>, DbErr> {
        tenant_select(PartQuery::find(), part_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl PartGateway {
    /// `PD-028`: every part of the caller's tenant, by name.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<part_entity::Model>, u64), DbErr> {
        let query = tenant_select(PartQuery::find(), part_entity::Column::TenantId)
            .order_by_asc(part_entity::Column::Name);
        fetch_page(query, &self.db, page, page_size).await
    }
}

/// `HRMS-800` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::part_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_parts() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(part_entity::Entity::find(), part_entity::Column::TenantId)
                .build(DbBackend::MySql)
                .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_part_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(part_entity::Entity::find(), part_entity::Column::TenantId)
                .build(DbBackend::MySql)
                .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_part_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                part_entity::Entity::delete_many(),
                part_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
