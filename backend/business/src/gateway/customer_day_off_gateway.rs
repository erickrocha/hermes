use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::business_error::BusinessError;
use crate::domain::customer_day_off::{CustomerDayOff, CustomerDayOffEntityMapper};
use entity::customer_day_off_entity;
use entity::prelude::CustomerDayOffEntity as DayOffQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder, SqlErr,
};

pub enum CustomerDayOffStoreError {
    DuplicateDate,
    Storage(BusinessError),
}

#[async_trait]
pub trait CustomerDayOffStore: Send + Sync {
    async fn save(
        &self,
        day_off: CustomerDayOff,
    ) -> Result<CustomerDayOff, CustomerDayOffStoreError>;
    async fn get_by_uuid(&self, uuid: String) -> Result<Option<CustomerDayOff>, BusinessError>;
    async fn delete_by_id(&self, id: i64) -> Result<(), BusinessError>;
    async fn list_page_by_customer(
        &self,
        customer_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<CustomerDayOff>, u64), BusinessError>;
}

/// `HRMS-601` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as `CustomerGateway`.
pub struct CustomerDayOffGateway {
    db: DbConn,
}

impl CustomerDayOffGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl CustomerDayOffStore for CustomerDayOffGateway {
    async fn save(
        &self,
        day_off: CustomerDayOff,
    ) -> Result<CustomerDayOff, CustomerDayOffStoreError> {
        let active_model = CustomerDayOffEntityMapper::build_active_model(day_off);
        let saved = active_model.save(&self.db).await.map_err(|error| {
            if matches!(error.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
                CustomerDayOffStoreError::DuplicateDate
            } else {
                CustomerDayOffStoreError::Storage(database_error(error))
            }
        })?;
        Ok(CustomerDayOffEntityMapper::from_active_model(saved))
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<CustomerDayOff>, BusinessError> {
        let model = <Self as Gateway<
            CustomerDayOff,
            customer_day_off_entity::Model,
            customer_day_off_entity::ActiveModel,
        >>::find_by_uuid(self, uuid)
        .await
        .map_err(database_error)?;
        Ok(model.map(CustomerDayOffEntityMapper::from_model))
    }

    async fn delete_by_id(&self, id: i64) -> Result<(), BusinessError> {
        <Self as Gateway<
            CustomerDayOff,
            customer_day_off_entity::Model,
            customer_day_off_entity::ActiveModel,
        >>::delete_by_id(self, id)
        .await
        .map_err(database_error)?;
        Ok(())
    }

    async fn list_page_by_customer(
        &self,
        customer_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<CustomerDayOff>, u64), BusinessError> {
        let (models, total) = CustomerDayOffGateway::find_page_by_customer(
            self,
            customer_id,
            page,
            page_size,
        )
        .await
        .map_err(database_error)?;
        Ok((CustomerDayOffEntityMapper::from_models(models), total))
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Customer day-off database error: {error}");
    log::error!("[CustomerDayOffGateway] {message}");
    BusinessError::new(message)
}

#[async_trait]
impl Gateway<CustomerDayOff, customer_day_off_entity::Model, customer_day_off_entity::ActiveModel>
    for CustomerDayOffGateway
{
    async fn persist(
        &self,
        entity: CustomerDayOff,
    ) -> Result<customer_day_off_entity::ActiveModel, DbErr> {
        CustomerDayOffEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            DayOffQuery::delete_many().filter(customer_day_off_entity::Column::Id.eq(id)),
            customer_day_off_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(
        &self,
        id: i64,
    ) -> Result<Option<customer_day_off_entity::Model>, DbErr> {
        tenant_select(DayOffQuery::find(), customer_day_off_entity::Column::TenantId)
            .filter(customer_day_off_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<customer_day_off_entity::Model>, DbErr> {
        tenant_select(DayOffQuery::find(), customer_day_off_entity::Column::TenantId)
            .filter(customer_day_off_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<customer_day_off_entity::Model>, DbErr> {
        tenant_select(DayOffQuery::find(), customer_day_off_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl CustomerDayOffGateway {
    /// `PD-028`: a customer's day-offs, newest first. Still tenant-scoped.
    pub async fn find_page_by_customer(
        &self,
        customer_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<customer_day_off_entity::Model>, u64), DbErr> {
        let query = tenant_select(DayOffQuery::find(), customer_day_off_entity::Column::TenantId)
            .filter(customer_day_off_entity::Column::CustomerId.eq(customer_id))
            .order_by_desc(customer_day_off_entity::Column::Date);
        fetch_page(query, &self.db, page, page_size).await
    }
}

impl CustomerDayOffGateway {
    /// `TRM-001`: the client day-offs on a date, for the effective schedule.
    pub async fn find_by_date(&self, date: chrono::NaiveDate) -> Result<Vec<customer_day_off_entity::Model>, DbErr> {
        tenant_select(DayOffQuery::find(), customer_day_off_entity::Column::TenantId)
            .filter(customer_day_off_entity::Column::Date.eq(date))
            .all(&self.db)
            .await
    }
}

/// `HRMS-601` (`D-09`): same technique `customer_gateway.rs`'s own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::customer_day_off_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_day_offs() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                customer_day_off_entity::Entity::find(),
                customer_day_off_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_day_off_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                customer_day_off_entity::Entity::find(),
                customer_day_off_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_day_off_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                customer_day_off_entity::Entity::delete_many(),
                customer_day_off_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
