use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::holiday::{Holiday, HolidayEntityMapper};
use entity::holiday_entity;
use entity::prelude::HolidayEntity as HolidayQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

/// `HRMS-602` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as `CustomerGateway`.
pub struct HolidayGateway {
    db: DbConn,
}

impl HolidayGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<Holiday, holiday_entity::Model, holiday_entity::ActiveModel> for HolidayGateway {
    async fn persist(&self, entity: Holiday) -> Result<holiday_entity::ActiveModel, DbErr> {
        HolidayEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            HolidayQuery::delete_many().filter(holiday_entity::Column::Id.eq(id)),
            holiday_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<holiday_entity::Model>, DbErr> {
        tenant_select(HolidayQuery::find(), holiday_entity::Column::TenantId)
            .filter(holiday_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<holiday_entity::Model>, DbErr> {
        tenant_select(HolidayQuery::find(), holiday_entity::Column::TenantId)
            .filter(holiday_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<holiday_entity::Model>, DbErr> {
        tenant_select(HolidayQuery::find(), holiday_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl HolidayGateway {
    /// `PD-028`.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<holiday_entity::Model>, u64), DbErr> {
        let query = tenant_select(HolidayQuery::find(), holiday_entity::Column::TenantId)
            .order_by_asc(holiday_entity::Column::Date);
        fetch_page(query, &self.db, page, page_size).await
    }
}

/// `HRMS-602` (`D-09`): same technique `customer_gateway.rs`'s own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::holiday_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_holidays() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(holiday_entity::Entity::find(), holiday_entity::Column::TenantId)
                .build(DbBackend::MySql)
                .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_holiday_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(holiday_entity::Entity::find(), holiday_entity::Column::TenantId)
                .build(DbBackend::MySql)
                .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_holiday_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                holiday_entity::Entity::delete_many(),
                holiday_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
