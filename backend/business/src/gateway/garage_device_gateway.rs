use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::business_error::BusinessError;
use crate::domain::garage_device::{GarageDevice, GarageDeviceEntityMapper};
use chrono::NaiveDateTime;
use entity::garage_device_entity;
use entity::prelude::GarageDeviceEntity as GarageDeviceQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

#[async_trait]
pub trait GarageDeviceStore: Send + Sync {
    async fn save(&self, device: GarageDevice) -> Result<GarageDevice, BusinessError>;
    async fn find_by_uuid(&self, uuid: String) -> Result<Option<GarageDevice>, BusinessError>;
    async fn find_by_pairing_code(
        &self,
        code: String,
    ) -> Result<Option<GarageDevice>, BusinessError>;
    async fn consume_pairing_code(
        &self,
        code: String,
        now: NaiveDateTime,
    ) -> Result<Option<GarageDevice>, BusinessError>;
    async fn find_for_authentication(&self, id: i64)
    -> Result<Option<GarageDevice>, BusinessError>;
    async fn list(&self) -> Result<Vec<GarageDevice>, BusinessError>;
}

pub struct GarageDeviceGateway {
    db: DbConn,
}

impl GarageDeviceGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Garage device database error: {error}");
    log::error!("[GarageDeviceGateway] {message}");
    BusinessError::new(message)
}

#[async_trait]
impl GarageDeviceStore for GarageDeviceGateway {
    async fn save(&self, device: GarageDevice) -> Result<GarageDevice, BusinessError> {
        let active = <Self as Gateway<
            GarageDevice,
            garage_device_entity::Model,
            garage_device_entity::ActiveModel,
        >>::persist(self, device)
        .await
        .map_err(database_error)?;
        Ok(GarageDeviceEntityMapper::from_active_model(active))
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<GarageDevice>, BusinessError> {
        let model = tenant_select(
            GarageDeviceQuery::find(),
            garage_device_entity::Column::TenantId,
        )
        .filter(garage_device_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
        .one(&self.db)
        .await
        .map_err(database_error)?;
        Ok(model.map(GarageDeviceEntityMapper::from_model))
    }

    async fn find_by_pairing_code(
        &self,
        code: String,
    ) -> Result<Option<GarageDevice>, BusinessError> {
        let model = GarageDeviceQuery::find()
            .filter(garage_device_entity::Column::PairingCode.eq(code))
            .one(&self.db)
            .await
            .map_err(database_error)?;
        Ok(model.map(GarageDeviceEntityMapper::from_model))
    }

    async fn consume_pairing_code(
        &self,
        code: String,
        now: NaiveDateTime,
    ) -> Result<Option<GarageDevice>, BusinessError> {
        let Some(model) = GarageDeviceQuery::find()
            .filter(garage_device_entity::Column::PairingCode.eq(code.clone()))
            .one(&self.db)
            .await
            .map_err(database_error)?
        else {
            return Ok(None);
        };
        let Some(tenant_id) = model.tenant_id else {
            return Ok(None);
        };
        let now_utc = now.and_utc();
        let update = || async {
            GarageDeviceQuery::update_many()
                .col_expr(
                    garage_device_entity::Column::PairingCode,
                    Expr::value(None::<String>),
                )
                .col_expr(
                    garage_device_entity::Column::PairingCodeExpiresAt,
                    Expr::value(None::<chrono::DateTime<chrono::Utc>>),
                )
                .col_expr(
                    garage_device_entity::Column::PairedAt,
                    Expr::value(Some(now_utc)),
                )
                .col_expr(
                    garage_device_entity::Column::UpdatedAt,
                    Expr::value(Some(now_utc)),
                )
                .col_expr(
                    garage_device_entity::Column::UpdatedBy,
                    Expr::value(Some("garage-device-pairing".to_string())),
                )
                .filter(garage_device_entity::Column::Id.eq(model.id))
                .filter(garage_device_entity::Column::TenantId.eq(tenant_id))
                .filter(garage_device_entity::Column::PairingCode.eq(code))
                .filter(garage_device_entity::Column::PairingCodeExpiresAt.gt(now_utc))
                .filter(garage_device_entity::Column::PairedAt.is_null())
                .filter(garage_device_entity::Column::RevokedAt.is_null())
                .exec(&self.db)
                .await
        };
        let result = entity::audit::run_for_tenant(tenant_id, "garage-device-pairing", update())
            .await
            .map_err(database_error)?;
        if result.rows_affected != 1 {
            return Ok(None);
        }
        Ok(Some(GarageDeviceEntityMapper::from_model(
            garage_device_entity::Model {
                pairing_code: None,
                pairing_code_expires_at: None,
                paired_at: Some(now_utc),
                updated_at: now_utc,
                updated_by: Some("garage-device-pairing".to_string()),
                ..model
            },
        )))
    }

    async fn find_for_authentication(
        &self,
        id: i64,
    ) -> Result<Option<GarageDevice>, BusinessError> {
        let model = GarageDeviceQuery::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(database_error)?;
        Ok(model.map(GarageDeviceEntityMapper::from_model))
    }

    async fn list(&self) -> Result<Vec<GarageDevice>, BusinessError> {
        let models = tenant_select(
            GarageDeviceQuery::find(),
            garage_device_entity::Column::TenantId,
        )
        .order_by_asc(garage_device_entity::Column::Name)
        .all(&self.db)
        .await
        .map_err(database_error)?;
        Ok(GarageDeviceEntityMapper::from_models(models))
    }
}

#[async_trait]
impl Gateway<GarageDevice, garage_device_entity::Model, garage_device_entity::ActiveModel>
    for GarageDeviceGateway
{
    async fn persist(
        &self,
        device: GarageDevice,
    ) -> Result<garage_device_entity::ActiveModel, DbErr> {
        GarageDeviceEntityMapper::build_active_model(device)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            GarageDeviceQuery::delete_many().filter(garage_device_entity::Column::Id.eq(id)),
            garage_device_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<garage_device_entity::Model>, DbErr> {
        tenant_select(
            GarageDeviceQuery::find(),
            garage_device_entity::Column::TenantId,
        )
        .filter(garage_device_entity::Column::Id.eq(id))
        .one(&self.db)
        .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<garage_device_entity::Model>, DbErr> {
        tenant_select(
            GarageDeviceQuery::find(),
            garage_device_entity::Column::TenantId,
        )
        .filter(garage_device_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
        .one(&self.db)
        .await
    }

    async fn find_all(&self) -> Result<Vec<garage_device_entity::Model>, DbErr> {
        tenant_select(
            GarageDeviceQuery::find(),
            garage_device_entity::Column::TenantId,
        )
        .all(&self.db)
        .await
    }
}
