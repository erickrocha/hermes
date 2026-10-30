use crate::commons::entity_mapper::EntityMapper;
use crate::domain::business_error::BusinessError;
use crate::domain::cross_tenant_action::{CrossTenantAction, CrossTenantActionEntityMapper};
use entity::cross_tenant_action_entity::Column;
use entity::prelude::CrossTenantActionEntity as Query;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, Condition, DbConn, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};

#[async_trait]
pub trait CrossTenantActionStore: Send + Sync {
    async fn record(&self, action: CrossTenantAction) -> Result<(), BusinessError>;
    /// The page of actions in which `tenant_id` is the affected tenant or the acting one, newest first, and the total.
    async fn list_page(&self, tenant_id: i64, page: u64, page_size: u64) -> Result<(Vec<CrossTenantAction>, u64), BusinessError>;
}

/// `HRMS-877`. The table belongs to two tenants and carries no `tenant_id`, so the tenant scope does not filter it;
/// every read names the tenant it is for.
pub struct CrossTenantActionGateway {
    db: DbConn,
}

impl CrossTenantActionGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

fn database_error(error: DbErr) -> BusinessError {
    BusinessError::new(format!("Database error: {error}"))
}

#[async_trait]
impl CrossTenantActionStore for CrossTenantActionGateway {
    async fn record(&self, action: CrossTenantAction) -> Result<(), BusinessError> {
        CrossTenantActionEntityMapper::build_active_model(action).insert(&self.db).await.map_err(database_error)?;
        Ok(())
    }

    async fn list_page(&self, tenant_id: i64, page: u64, page_size: u64) -> Result<(Vec<CrossTenantAction>, u64), BusinessError> {
        let involved = Condition::any()
            .add(Column::AffectedTenantId.eq(tenant_id as i32))
            .add(Column::ActorTenantId.eq(tenant_id as i32));
        let paginator = Query::find().filter(involved).order_by_desc(Column::OccurredAt).order_by_desc(Column::Id).paginate(&self.db, page_size);
        let total = paginator.num_items().await.map_err(database_error)?;
        let models = paginator.fetch_page(page).await.map_err(database_error)?;
        Ok((CrossTenantActionEntityMapper::from_models(models), total))
    }
}
