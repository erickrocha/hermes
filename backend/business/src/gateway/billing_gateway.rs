use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_select};
use crate::domain::billing::{
    BillingCharge, BillingChargeEntityMapper, TenantBilling, TenantBillingEntityMapper,
};
use crate::domain::business_error::BusinessError;
use chrono::NaiveDateTime;
use entity::billing_charge_entity;
use entity::prelude::{BillingChargeEntity as ChargeQuery, TenantBillingEntity as BillingQuery, UserEntity as UserQuery};
use entity::tenant_billing_entity;
use entity::user_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};

#[async_trait]
pub trait BillingStore: Send + Sync {
    async fn get_settings(&self, tenant_id: i64) -> Result<Option<TenantBilling>, BusinessError>;
    async fn save_settings(&self, settings: TenantBilling) -> Result<TenantBilling, BusinessError>;
    async fn count_active_human_users(&self, tenant_id: i64) -> Result<i32, BusinessError>;
    async fn find_charge_by_uuid(&self, uuid: &str) -> Result<Option<BillingCharge>, BusinessError>;
    async fn find_charge_for_period(
        &self,
        tenant_id: i64,
        period_start: NaiveDateTime,
    ) -> Result<Option<BillingCharge>, BusinessError>;
    async fn save_charge(&self, charge: BillingCharge) -> Result<BillingCharge, BusinessError>;
    async fn list_charges(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<BillingCharge>, u64), BusinessError>;
}

pub struct BillingGateway {
    db: DbConn,
}

impl BillingGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Billing database error: {error}");
    log::error!("[BillingGateway] {message}");
    BusinessError::new(message)
}

#[async_trait]
impl BillingStore for BillingGateway {
    async fn get_settings(&self, tenant_id: i64) -> Result<Option<TenantBilling>, BusinessError> {
        let model = tenant_select(BillingQuery::find(), tenant_billing_entity::Column::TenantId)
            .filter(tenant_billing_entity::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(database_error)?;
        Ok(model.map(TenantBillingEntityMapper::from_model))
    }

    async fn save_settings(&self, settings: TenantBilling) -> Result<TenantBilling, BusinessError> {
        let active = TenantBillingEntityMapper::build_active_model(settings);
        active
            .save(&self.db)
            .await
            .map(TenantBillingEntityMapper::from_active_model)
            .map_err(database_error)
    }

    async fn count_active_human_users(&self, tenant_id: i64) -> Result<i32, BusinessError> {
        let count = tenant_select(UserQuery::find(), user_entity::Column::TenantId)
            .filter(user_entity::Column::TenantId.eq(tenant_id))
            .filter(user_entity::Column::Enabled.eq(true))
            .filter(user_entity::Column::Role.is_in(["TenantOwner", "TenantUser", "Driver", "Mechanic"]))
            .count(&self.db)
            .await
            .map_err(database_error)?;
        i32::try_from(count).map_err(|_| BusinessError::new("Active user count exceeds supported range".to_string()))
    }

    async fn find_charge_by_uuid(&self, uuid: &str) -> Result<Option<BillingCharge>, BusinessError> {
        let model = tenant_select(ChargeQuery::find(), billing_charge_entity::Column::TenantId)
            .filter(billing_charge_entity::Column::Uuid.eq(string_to_bytes(uuid)))
            .one(&self.db)
            .await
            .map_err(database_error)?;
        Ok(model.map(BillingChargeEntityMapper::from_model))
    }

    async fn find_charge_for_period(
        &self,
        tenant_id: i64,
        period_start: NaiveDateTime,
    ) -> Result<Option<BillingCharge>, BusinessError> {
        let model = tenant_select(ChargeQuery::find(), billing_charge_entity::Column::TenantId)
            .filter(billing_charge_entity::Column::TenantId.eq(tenant_id))
            .filter(billing_charge_entity::Column::PeriodStart.eq(period_start.and_utc()))
            .one(&self.db)
            .await
            .map_err(database_error)?;
        Ok(model.map(BillingChargeEntityMapper::from_model))
    }

    async fn save_charge(&self, charge: BillingCharge) -> Result<BillingCharge, BusinessError> {
        let active = BillingChargeEntityMapper::build_active_model(charge);
        active
            .save(&self.db)
            .await
            .map(BillingChargeEntityMapper::from_active_model)
            .map_err(database_error)
    }

    async fn list_charges(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<BillingCharge>, u64), BusinessError> {
        let query = tenant_select(ChargeQuery::find(), billing_charge_entity::Column::TenantId)
            .order_by_desc(billing_charge_entity::Column::PeriodEnd)
            .order_by_desc(billing_charge_entity::Column::Id);
        fetch_page(query, &self.db, page, page_size)
            .await
            .map(|(models, total)| (BillingChargeEntityMapper::from_models(models), total))
            .map_err(database_error)
    }
}

#[async_trait]
impl Gateway<TenantBilling, tenant_billing_entity::Model, tenant_billing_entity::ActiveModel>
    for BillingGateway
{
    async fn persist(
        &self,
        entity: TenantBilling,
    ) -> Result<tenant_billing_entity::ActiveModel, DbErr> {
        TenantBillingEntityMapper::build_active_model(entity).save(&self.db).await
    }

    async fn delete_by_id(&self, id: i64) -> Result<sea_orm::DeleteResult, DbErr> {
        tenant_delete_billing_settings(&self.db, id).await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<tenant_billing_entity::Model>, DbErr> {
        tenant_select(BillingQuery::find(), tenant_billing_entity::Column::TenantId)
            .filter(tenant_billing_entity::Column::TenantId.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<tenant_billing_entity::Model>, DbErr> {
        tenant_select(BillingQuery::find(), tenant_billing_entity::Column::TenantId)
            .filter(tenant_billing_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<tenant_billing_entity::Model>, DbErr> {
        tenant_select(BillingQuery::find(), tenant_billing_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

async fn tenant_delete_billing_settings(db: &DbConn, tenant_id: i64) -> Result<sea_orm::DeleteResult, DbErr> {
    use crate::commons::gateway::tenant_delete;
    tenant_delete(
        BillingQuery::delete_many().filter(tenant_billing_entity::Column::TenantId.eq(tenant_id)),
        tenant_billing_entity::Column::TenantId,
    )
    .exec(db)
    .await
}