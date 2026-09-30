use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::fuel_gauge_setting::{FuelGaugeSetting, FuelGaugeSettingEntityMapper};
use entity::fuel_gauge_setting_entity;
use entity::prelude::FuelGaugeSettingEntity as FuelGaugeSettingQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter};

/// `HRMS-953` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct FuelGaugeSettingGateway {
    db: DbConn,
}

impl FuelGaugeSettingGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<FuelGaugeSetting, fuel_gauge_setting_entity::Model, fuel_gauge_setting_entity::ActiveModel>
    for FuelGaugeSettingGateway
{
    async fn persist(&self, entity: FuelGaugeSetting) -> Result<fuel_gauge_setting_entity::ActiveModel, DbErr> {
        FuelGaugeSettingEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            FuelGaugeSettingQuery::delete_many().filter(fuel_gauge_setting_entity::Column::Id.eq(id)),
            fuel_gauge_setting_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<fuel_gauge_setting_entity::Model>, DbErr> {
        tenant_select(FuelGaugeSettingQuery::find(), fuel_gauge_setting_entity::Column::TenantId)
            .filter(fuel_gauge_setting_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<fuel_gauge_setting_entity::Model>, DbErr> {
        tenant_select(FuelGaugeSettingQuery::find(), fuel_gauge_setting_entity::Column::TenantId)
            .filter(fuel_gauge_setting_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<fuel_gauge_setting_entity::Model>, DbErr> {
        tenant_select(FuelGaugeSettingQuery::find(), fuel_gauge_setting_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl FuelGaugeSettingGateway {
    /// `the tenant's one row`: at most one row per tenant. For a tenant-bound caller,
    /// `tenant_select`'s own ambient scope already narrows this to "the
    /// caller's own"; `target_tenant_id` exists so an **unbound** `SysAdmin`
    /// -- whose scope is otherwise unrestricted across every tenant -- names
    /// which tenant's singleton row it means, the same way every create
    /// endpoint already lets it name a target tenant explicitly. Without it,
    /// an unbound caller's "the one row" would be ambiguous the moment a
    /// second tenant configures its own tank.
    pub async fn find_current(
        &self,
        target_tenant_id: Option<i64>,
    ) -> Result<Option<fuel_gauge_setting_entity::Model>, DbErr> {
        let mut query = tenant_select(FuelGaugeSettingQuery::find(), fuel_gauge_setting_entity::Column::TenantId);
        if let Some(tenant_id) = target_tenant_id {
            query = query.filter(fuel_gauge_setting_entity::Column::TenantId.eq(tenant_id));
        }
        query.one(&self.db).await
    }
}
