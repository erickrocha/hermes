use crate::commons::gateway::tenant_select;
use crate::domain::business_error::BusinessError;
use crate::domain::tenant_tracking_setting::TrackingCredentials;
use entity::prelude::TenantTrackingSettingEntity as TrackingQuery;
use entity::tenant_tracking_setting_entity::{ActiveModel, Column};
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, DbErr, EntityTrait, QueryFilter, Set};

#[async_trait]
pub trait TenantTrackingSettingStore: Send + Sync {
    async fn get(&self, tenant_id: i64) -> Result<Option<TrackingCredentials>, BusinessError>;
    async fn save(&self, tenant_id: i64, credentials: TrackingCredentials) -> Result<(), BusinessError>;
}

/// `HRMS-900`..`905`: one row per tenant, read through `tenant_select` like every other gateway.
pub struct TenantTrackingSettingGateway {
    db: DbConn,
}

impl TenantTrackingSettingGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Tenant tracking setting database error: {error}");
    log::error!("[TenantTrackingSettingGateway] {message}");
    BusinessError::new(message)
}

#[async_trait]
impl TenantTrackingSettingStore for TenantTrackingSettingGateway {
    async fn get(&self, tenant_id: i64) -> Result<Option<TrackingCredentials>, BusinessError> {
        let row = tenant_select(TrackingQuery::find(), Column::TenantId)
            .filter(Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(database_error)?;
        Ok(row.map(|row| TrackingCredentials {
            base_url: row.base_url,
            email: row.email,
            password: row.password,
        }))
    }

    async fn save(&self, tenant_id: i64, credentials: TrackingCredentials) -> Result<(), BusinessError> {
        let existing = tenant_select(TrackingQuery::find(), Column::TenantId)
            .filter(Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(database_error)?;
        let mut row = match existing {
            Some(existing) => existing.into(),
            None => ActiveModel { tenant_id: Set(Some(tenant_id)), ..Default::default() },
        };
        row.base_url = Set(credentials.base_url);
        row.email = Set(credentials.email);
        row.password = Set(credentials.password);
        row.save(&self.db).await.map_err(database_error)?;
        Ok(())
    }
}
