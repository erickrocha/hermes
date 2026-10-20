use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::business_error::BusinessError;
use crate::domain::maintenance_notification::{
    MaintenanceNotification, MaintenanceNotificationEntityMapper,
};
use chrono::NaiveDateTime;
use entity::maintenance_notification_entity;
use entity::prelude::MaintenanceNotificationEntity as MaintenanceNotificationQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect,
};

#[async_trait]
pub trait MaintenanceNotificationStore: Send + Sync {
    async fn save(
        &self,
        notification: MaintenanceNotification,
    ) -> Result<MaintenanceNotification, BusinessError>;
    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<MaintenanceNotification>, BusinessError>;
    /// Most recent first; `unread_only` keeps just the ones with no `read_at`.
    async fn find_page(
        &self,
        unread_only: bool,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<MaintenanceNotification>, u64), BusinessError>;
    async fn count_unread(&self) -> Result<u64, BusinessError>;
    /// Marks every unread notice visible to the caller; returns how many.
    async fn mark_all_read(&self, now: NaiveDateTime) -> Result<u64, BusinessError>;
}

/// `EPIC-MT-06`: every read goes through `tenant_select`, same as every other gateway.
pub struct MaintenanceNotificationGateway {
    db: DbConn,
}

impl MaintenanceNotificationGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    fn unread(&self) -> sea_orm::Select<MaintenanceNotificationQuery> {
        tenant_select(
            MaintenanceNotificationQuery::find(),
            maintenance_notification_entity::Column::TenantId,
        )
        .filter(maintenance_notification_entity::Column::ReadAt.is_null())
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Database error: {error}");
    log::error!("[MaintenanceNotificationGateway] {message}");
    BusinessError::new(message)
}

#[async_trait]
impl MaintenanceNotificationStore for MaintenanceNotificationGateway {
    async fn save(
        &self,
        notification: MaintenanceNotification,
    ) -> Result<MaintenanceNotification, BusinessError> {
        self.persist(notification)
            .await
            .map(MaintenanceNotificationEntityMapper::from_active_model)
            .map_err(database_error)
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<MaintenanceNotification>, BusinessError> {
        Gateway::find_by_uuid(self, uuid)
            .await
            .map(|model| model.map(MaintenanceNotificationEntityMapper::from_model))
            .map_err(database_error)
    }

    async fn find_page(
        &self,
        unread_only: bool,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<MaintenanceNotification>, u64), BusinessError> {
        let query = if unread_only {
            self.unread()
        } else {
            tenant_select(
                MaintenanceNotificationQuery::find(),
                maintenance_notification_entity::Column::TenantId,
            )
        }
        .order_by_desc(maintenance_notification_entity::Column::CreatedAt)
        .order_by_desc(maintenance_notification_entity::Column::Id);
        fetch_page(query, &self.db, page, page_size)
            .await
            .map(|(models, total)| {
                (
                    MaintenanceNotificationEntityMapper::from_models(models),
                    total,
                )
            })
            .map_err(database_error)
    }

    async fn count_unread(&self) -> Result<u64, BusinessError> {
        self.unread().count(&self.db).await.map_err(database_error)
    }

    async fn mark_all_read(&self, now: NaiveDateTime) -> Result<u64, BusinessError> {
        let ids: Vec<i64> = self
            .unread()
            .select_only()
            .column(maintenance_notification_entity::Column::Id)
            .into_tuple()
            .all(&self.db)
            .await
            .map_err(database_error)?;
        if ids.is_empty() {
            return Ok(0);
        }
        let result = MaintenanceNotificationQuery::update_many()
            .col_expr(
                maintenance_notification_entity::Column::ReadAt,
                Expr::value(Some(now.and_utc())),
            )
            .filter(maintenance_notification_entity::Column::Id.is_in(ids))
            .filter(maintenance_notification_entity::Column::ReadAt.is_null())
            .exec(&self.db)
            .await
            .map_err(database_error)?;
        Ok(result.rows_affected)
    }
}

#[async_trait]
impl
    Gateway<
        MaintenanceNotification,
        maintenance_notification_entity::Model,
        maintenance_notification_entity::ActiveModel,
    > for MaintenanceNotificationGateway
{
    async fn persist(
        &self,
        entity: MaintenanceNotification,
    ) -> Result<maintenance_notification_entity::ActiveModel, DbErr> {
        MaintenanceNotificationEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            MaintenanceNotificationQuery::delete_many()
                .filter(maintenance_notification_entity::Column::Id.eq(id)),
            maintenance_notification_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(
        &self,
        id: i64,
    ) -> Result<Option<maintenance_notification_entity::Model>, DbErr> {
        tenant_select(
            MaintenanceNotificationQuery::find(),
            maintenance_notification_entity::Column::TenantId,
        )
        .filter(maintenance_notification_entity::Column::Id.eq(id))
        .one(&self.db)
        .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<maintenance_notification_entity::Model>, DbErr> {
        tenant_select(
            MaintenanceNotificationQuery::find(),
            maintenance_notification_entity::Column::TenantId,
        )
        .filter(maintenance_notification_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
        .one(&self.db)
        .await
    }

    async fn find_all(&self) -> Result<Vec<maintenance_notification_entity::Model>, DbErr> {
        tenant_select(
            MaintenanceNotificationQuery::find(),
            maintenance_notification_entity::Column::TenantId,
        )
        .all(&self.db)
        .await
    }
}
