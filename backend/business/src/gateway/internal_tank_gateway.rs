use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::internal_tank::{InternalTank, InternalTankEntityMapper};
use entity::internal_tank_entity;
use entity::prelude::InternalTankEntity as InternalTankQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter};

/// `HRMS-943` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct InternalTankGateway {
    db: DbConn,
}

impl InternalTankGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<InternalTank, internal_tank_entity::Model, internal_tank_entity::ActiveModel>
    for InternalTankGateway
{
    async fn persist(&self, entity: InternalTank) -> Result<internal_tank_entity::ActiveModel, DbErr> {
        InternalTankEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            InternalTankQuery::delete_many().filter(internal_tank_entity::Column::Id.eq(id)),
            internal_tank_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<internal_tank_entity::Model>, DbErr> {
        tenant_select(InternalTankQuery::find(), internal_tank_entity::Column::TenantId)
            .filter(internal_tank_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<internal_tank_entity::Model>, DbErr> {
        tenant_select(InternalTankQuery::find(), internal_tank_entity::Column::TenantId)
            .filter(internal_tank_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<internal_tank_entity::Model>, DbErr> {
        tenant_select(InternalTankQuery::find(), internal_tank_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl InternalTankGateway {
    /// `TRM-1540`: at most one row per tenant. For a tenant-bound caller,
    /// `tenant_select`'s own ambient scope already narrows this to "the
    /// caller's own"; `target_tenant_id` exists so an **unbound** `SysAdmin`
    /// -- whose scope is otherwise unrestricted across every tenant -- names
    /// which tenant's singleton row it means, the same way every create
    /// endpoint already lets it name a target tenant explicitly. Without it,
    /// an unbound caller's "the one row" would be ambiguous the moment a
    /// second tenant configures its own tank.
    pub async fn find_current(
        &self,
        target_tenant_id: Option<i64>,
    ) -> Result<Option<internal_tank_entity::Model>, DbErr> {
        let mut query = tenant_select(InternalTankQuery::find(), internal_tank_entity::Column::TenantId);
        if let Some(tenant_id) = target_tenant_id {
            query = query.filter(internal_tank_entity::Column::TenantId.eq(tenant_id));
        }
        query.one(&self.db).await
    }
}

/// `HRMS-943` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::internal_tank_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_internal_tank() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(internal_tank_entity::Entity::find(), internal_tank_entity::Column::TenantId)
                .build(DbBackend::MySql)
                .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_internal_tank_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(internal_tank_entity::Entity::find(), internal_tank_entity::Column::TenantId)
                .build(DbBackend::MySql)
                .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_an_internal_tank_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                internal_tank_entity::Entity::delete_many(),
                internal_tank_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
