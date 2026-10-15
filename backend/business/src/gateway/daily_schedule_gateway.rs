use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::business_error::BusinessError;
use crate::domain::daily_schedule::{DailySchedule, DailyScheduleEntityMapper};
use entity::daily_schedule_entity;
use entity::prelude::DailyScheduleEntity as ScheduleQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder, SqlErr,
};

#[async_trait]
pub trait DailyScheduleStore: Send + Sync {
    async fn save(&self, entry: DailySchedule) -> Result<DailySchedule, DailyScheduleStoreError>;
    async fn get_by_uuid(&self, uuid: String) -> Result<Option<DailySchedule>, BusinessError>;
    async fn delete_by_id(&self, id: i64) -> Result<(), BusinessError>;
    async fn list_page_by_demand(
        &self,
        demand_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<DailySchedule>, u64), BusinessError>;
}

pub enum DailyScheduleStoreError {
    Duplicate,
    Storage(BusinessError),
}

/// `HRMS-605` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct DailyScheduleGateway {
    db: DbConn,
}

impl DailyScheduleGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DailyScheduleStore for DailyScheduleGateway {
    async fn save(&self, entry: DailySchedule) -> Result<DailySchedule, DailyScheduleStoreError> {
        let active = <Self as Gateway<
            DailySchedule,
            daily_schedule_entity::Model,
            daily_schedule_entity::ActiveModel,
        >>::persist(self, entry)
        .await
        .map_err(|error| match error.sql_err() {
            Some(SqlErr::UniqueConstraintViolation(_)) => DailyScheduleStoreError::Duplicate,
            _ => DailyScheduleStoreError::Storage(database_error(error)),
        })?;
        Ok(DailyScheduleEntityMapper::from_active_model(active))
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<DailySchedule>, BusinessError> {
        <Self as Gateway<
            DailySchedule,
            daily_schedule_entity::Model,
            daily_schedule_entity::ActiveModel,
        >>::find_by_uuid(self, uuid)
        .await
        .map(|row| row.map(DailyScheduleEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn delete_by_id(&self, id: i64) -> Result<(), BusinessError> {
        <Self as Gateway<
            DailySchedule,
            daily_schedule_entity::Model,
            daily_schedule_entity::ActiveModel,
        >>::delete_by_id(self, id)
        .await
        .map(|_| ())
        .map_err(database_error)
    }

    async fn list_page_by_demand(
        &self,
        demand_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<DailySchedule>, u64), BusinessError> {
        self.find_page_by_demand(demand_id, page, page_size)
            .await
            .map(|(rows, total)| (DailyScheduleEntityMapper::from_models(rows), total))
            .map_err(database_error)
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Database error: {error}");
    log::error!("[DailyScheduleGateway] {message}");
    BusinessError::new(message)
}

#[async_trait]
impl Gateway<DailySchedule, daily_schedule_entity::Model, daily_schedule_entity::ActiveModel>
    for DailyScheduleGateway
{
    async fn persist(
        &self,
        entity: DailySchedule,
    ) -> Result<daily_schedule_entity::ActiveModel, DbErr> {
        DailyScheduleEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            ScheduleQuery::delete_many().filter(daily_schedule_entity::Column::Id.eq(id)),
            daily_schedule_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<daily_schedule_entity::Model>, DbErr> {
        tenant_select(
            ScheduleQuery::find(),
            daily_schedule_entity::Column::TenantId,
        )
        .filter(daily_schedule_entity::Column::Id.eq(id))
        .one(&self.db)
        .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<daily_schedule_entity::Model>, DbErr> {
        tenant_select(
            ScheduleQuery::find(),
            daily_schedule_entity::Column::TenantId,
        )
        .filter(daily_schedule_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
        .one(&self.db)
        .await
    }

    async fn find_all(&self) -> Result<Vec<daily_schedule_entity::Model>, DbErr> {
        tenant_select(
            ScheduleQuery::find(),
            daily_schedule_entity::Column::TenantId,
        )
        .all(&self.db)
        .await
    }
}

impl DailyScheduleGateway {
    /// `PD-028`: a demand's manual day entries, newest first.
    pub async fn find_page_by_demand(
        &self,
        demand_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<daily_schedule_entity::Model>, u64), DbErr> {
        let query = tenant_select(
            ScheduleQuery::find(),
            daily_schedule_entity::Column::TenantId,
        )
        .filter(daily_schedule_entity::Column::DemandId.eq(demand_id))
        .order_by_desc(daily_schedule_entity::Column::Date);
        fetch_page(query, &self.db, page, page_size).await
    }
}

impl DailyScheduleGateway {
    /// `TRM-001`: the manual day entries for a date, for the effective schedule.
    pub async fn find_by_date(
        &self,
        date: chrono::NaiveDate,
    ) -> Result<Vec<daily_schedule_entity::Model>, DbErr> {
        tenant_select(
            ScheduleQuery::find(),
            daily_schedule_entity::Column::TenantId,
        )
        .filter(daily_schedule_entity::Column::Date.eq(date))
        .all(&self.db)
        .await
    }
}

/// `HRMS-605` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::daily_schedule_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_schedule_entries() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                daily_schedule_entity::Entity::find(),
                daily_schedule_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_schedule_entry_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                daily_schedule_entity::Entity::find(),
                daily_schedule_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_schedule_entry_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                daily_schedule_entity::Entity::delete_many(),
                daily_schedule_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
