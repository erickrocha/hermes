use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::tenant_rule_setting::{RuleSettings, TenantRuleSetting, TenantRuleSettingEntityMapper};
use entity::tenant_rule_setting_entity;
use entity::prelude::TenantRuleSettingEntity as TenantRuleSettingQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter};

/// `HRMS-954` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct TenantRuleSettingGateway {
    db: DbConn,
}

impl TenantRuleSettingGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<TenantRuleSetting, tenant_rule_setting_entity::Model, tenant_rule_setting_entity::ActiveModel>
    for TenantRuleSettingGateway
{
    async fn persist(&self, entity: TenantRuleSetting) -> Result<tenant_rule_setting_entity::ActiveModel, DbErr> {
        TenantRuleSettingEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            TenantRuleSettingQuery::delete_many().filter(tenant_rule_setting_entity::Column::Id.eq(id)),
            tenant_rule_setting_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<tenant_rule_setting_entity::Model>, DbErr> {
        tenant_select(TenantRuleSettingQuery::find(), tenant_rule_setting_entity::Column::TenantId)
            .filter(tenant_rule_setting_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<tenant_rule_setting_entity::Model>, DbErr> {
        tenant_select(TenantRuleSettingQuery::find(), tenant_rule_setting_entity::Column::TenantId)
            .filter(tenant_rule_setting_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<tenant_rule_setting_entity::Model>, DbErr> {
        tenant_select(TenantRuleSettingQuery::find(), tenant_rule_setting_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl TenantRuleSettingGateway {
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
    ) -> Result<Option<tenant_rule_setting_entity::Model>, DbErr> {
        let mut query = tenant_select(TenantRuleSettingQuery::find(), tenant_rule_setting_entity::Column::TenantId);
        if let Some(tenant_id) = target_tenant_id {
            query = query.filter(tenant_rule_setting_entity::Column::TenantId.eq(tenant_id));
        }
        query.one(&self.db).await
    }

    /// The thresholds in force for a tenant: its own row, else the legacy defaults.
    pub async fn settings_for(&self, tenant_id: Option<i64>) -> Result<RuleSettings, DbErr> {
        Ok(self
            .find_current(tenant_id)
            .await?
            .map(|m| RuleSettings::from(&m))
            .unwrap_or_default())
    }
}
