use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::business_error::BusinessError;
use crate::domain::customer::{Customer, CustomerEntityMapper};
use entity::customer_entity;
use entity::prelude::CustomerEntity as CustomerQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DbConn, DbErr, DeleteResult, EntityTrait,
    QueryFilter, QueryOrder,
};

#[async_trait]
pub trait CustomerStore: Send + Sync {
    async fn save(&self, customer: Customer) -> Result<Customer, BusinessError>;
    async fn get_by_id(&self, id: i64) -> Result<Option<Customer>, BusinessError>;
    async fn get_by_uuid(&self, uuid: String) -> Result<Option<Customer>, BusinessError>;
    /// The tenant's customer holding this (normalised) taxpayer document, if any -- under the caller's scope.
    async fn get_by_tax_id(&self, tenant_id: Option<i64>, tax_id: &str) -> Result<Option<Customer>, BusinessError>;
    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<Customer>, u64), BusinessError>;
}

/// `HRMS-600` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete` -- `VehicleGateway` is the reference
/// implementation this file copies, and `tenant_scoping_rule.rs` is what
/// refuses a gateway that skips either call.
pub struct CustomerGateway {
    db: DbConn,
}

impl CustomerGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl CustomerStore for CustomerGateway {
    async fn save(&self, customer: Customer) -> Result<Customer, BusinessError> {
        let entity = <Self as Gateway<
            Customer,
            customer_entity::Model,
            customer_entity::ActiveModel,
        >>::persist(self, customer)
        .await
        .map_err(database_error)?;
        Ok(CustomerEntityMapper::from_active_model(entity))
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<Customer>, BusinessError> {
        let model = <Self as Gateway<
            Customer,
            customer_entity::Model,
            customer_entity::ActiveModel,
        >>::find_by_id(self, id)
        .await
        .map_err(database_error)?;
        Ok(model.map(CustomerEntityMapper::from_model))
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<Customer>, BusinessError> {
        let model = <Self as Gateway<
            Customer,
            customer_entity::Model,
            customer_entity::ActiveModel,
        >>::find_by_uuid(self, uuid)
        .await
        .map_err(database_error)?;
        Ok(model.map(CustomerEntityMapper::from_model))
    }

    async fn get_by_tax_id(&self, tenant_id: Option<i64>, tax_id: &str) -> Result<Option<Customer>, BusinessError> {
        let model = tenant_select(CustomerQuery::find(), customer_entity::Column::TenantId)
            .filter(customer_entity::Column::TenantId.eq(tenant_id))
            .filter(customer_entity::Column::TaxId.eq(tax_id))
            .one(&self.db)
            .await
            .map_err(database_error)?;
        Ok(model.map(CustomerEntityMapper::from_model))
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<Customer>, u64), BusinessError> {
        let (models, total) = CustomerGateway::find_page(self, page, page_size, search)
            .await
            .map_err(database_error)?;
        Ok((CustomerEntityMapper::from_models(models), total))
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Customer database error: {error}");
    log::error!("[CustomerGateway] {message}");
    BusinessError::new(message)
}

#[async_trait]
impl Gateway<Customer, customer_entity::Model, customer_entity::ActiveModel> for CustomerGateway {
    async fn persist(&self, entity: Customer) -> Result<customer_entity::ActiveModel, DbErr> {
        let active_model = CustomerEntityMapper::build_active_model(entity);
        active_model.save(&self.db).await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            CustomerQuery::delete_many().filter(customer_entity::Column::Id.eq(id)),
            customer_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<customer_entity::Model>, DbErr> {
        tenant_select(CustomerQuery::find(), customer_entity::Column::TenantId)
            .filter(customer_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<customer_entity::Model>, DbErr> {
        tenant_select(CustomerQuery::find(), customer_entity::Column::TenantId)
            .filter(customer_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<customer_entity::Model>, DbErr> {
        tenant_select(CustomerQuery::find(), customer_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl CustomerGateway {
    /// PD-028. Still goes through `tenant_select`: paging must not be a read
    /// path that escapes the tenant scope.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<customer_entity::Model>, u64), DbErr> {
        let mut query = tenant_select(CustomerQuery::find(), customer_entity::Column::TenantId)
            .order_by_desc(customer_entity::Column::Id);
        if let Some(term) = search {
            query = query.filter(
                Condition::any()
                    .add(customer_entity::Column::Name.contains(term))
                    .add(customer_entity::Column::LegalName.contains(term))
                    .add(customer_entity::Column::TaxId.contains(term)),
            );
        }
        fetch_page(query, &self.db, page, page_size).await
    }
}

/// `HRMS-600` (`D-09`): the read side of the scoping rule, proved on the
/// query builder rather than against a database -- same technique
/// `vehicle_gateway.rs`'s own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::customer_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_customers() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                customer_entity::Entity::find(),
                customer_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_customer_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                customer_entity::Entity::find(),
                customer_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_customer_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                customer_entity::Entity::delete_many(),
                customer_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
