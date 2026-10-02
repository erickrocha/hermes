use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::preventive_plan_alert::{PreventivePlanAlert, PreventivePlanAlertEntityMapper};
use entity::prelude::PreventivePlanAlertEntity as AlertQuery;
use entity::preventive_plan_alert_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter, QueryOrder};

/// `HRMS-713` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`.
pub struct PreventivePlanAlertGateway {
    db: DbConn,
}

impl PreventivePlanAlertGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    /// `TRM-324`: a plan's alerts in ascending kilometre order.
    pub async fn find_by_plan(&self, plan_id: i64) -> Result<Vec<preventive_plan_alert_entity::Model>, DbErr> {
        tenant_select(AlertQuery::find(), preventive_plan_alert_entity::Column::TenantId)
            .filter(preventive_plan_alert_entity::Column::PreventivePlanId.eq(plan_id))
            .order_by_asc(preventive_plan_alert_entity::Column::AtKm)
            .all(&self.db)
            .await
    }
}

#[async_trait]
impl Gateway<PreventivePlanAlert, preventive_plan_alert_entity::Model, preventive_plan_alert_entity::ActiveModel>
    for PreventivePlanAlertGateway
{
    async fn persist(
        &self,
        entity: PreventivePlanAlert,
    ) -> Result<preventive_plan_alert_entity::ActiveModel, DbErr> {
        PreventivePlanAlertEntityMapper::build_active_model(entity).save(&self.db).await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            AlertQuery::delete_many().filter(preventive_plan_alert_entity::Column::Id.eq(id)),
            preventive_plan_alert_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<preventive_plan_alert_entity::Model>, DbErr> {
        tenant_select(AlertQuery::find(), preventive_plan_alert_entity::Column::TenantId)
            .filter(preventive_plan_alert_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<preventive_plan_alert_entity::Model>, DbErr> {
        tenant_select(AlertQuery::find(), preventive_plan_alert_entity::Column::TenantId)
            .filter(preventive_plan_alert_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<preventive_plan_alert_entity::Model>, DbErr> {
        tenant_select(AlertQuery::find(), preventive_plan_alert_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}
