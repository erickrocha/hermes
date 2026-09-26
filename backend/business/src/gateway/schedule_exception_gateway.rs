use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::schedule_exception::{ScheduleException, ScheduleExceptionEntityMapper};
use entity::prelude::ScheduleExceptionEntity as ExceptionQuery;
use entity::schedule_exception_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

/// `HRMS-606` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct ScheduleExceptionGateway {
    db: DbConn,
}

impl ScheduleExceptionGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<ScheduleException, schedule_exception_entity::Model, schedule_exception_entity::ActiveModel>
    for ScheduleExceptionGateway
{
    async fn persist(
        &self,
        entity: ScheduleException,
    ) -> Result<schedule_exception_entity::ActiveModel, DbErr> {
        ScheduleExceptionEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            ExceptionQuery::delete_many().filter(schedule_exception_entity::Column::Id.eq(id)),
            schedule_exception_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(
        &self,
        id: i64,
    ) -> Result<Option<schedule_exception_entity::Model>, DbErr> {
        tenant_select(ExceptionQuery::find(), schedule_exception_entity::Column::TenantId)
            .filter(schedule_exception_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<schedule_exception_entity::Model>, DbErr> {
        tenant_select(ExceptionQuery::find(), schedule_exception_entity::Column::TenantId)
            .filter(schedule_exception_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<schedule_exception_entity::Model>, DbErr> {
        tenant_select(ExceptionQuery::find(), schedule_exception_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl ScheduleExceptionGateway {
    /// `PD-028`: a demand's exceptions, newest first.
    pub async fn find_page_by_demand(
        &self,
        demand_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<schedule_exception_entity::Model>, u64), DbErr> {
        let query = tenant_select(ExceptionQuery::find(), schedule_exception_entity::Column::TenantId)
            .filter(schedule_exception_entity::Column::DemandId.eq(demand_id))
            .order_by_desc(schedule_exception_entity::Column::Date);
        fetch_page(query, &self.db, page, page_size).await
    }
}

/// `HRMS-606` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::schedule_exception_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_exceptions() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                schedule_exception_entity::Entity::find(),
                schedule_exception_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_exception_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                schedule_exception_entity::Entity::find(),
                schedule_exception_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_an_exception_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                schedule_exception_entity::Entity::delete_many(),
                schedule_exception_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
