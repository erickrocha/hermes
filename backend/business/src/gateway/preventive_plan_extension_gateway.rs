use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::preventive_plan_extension::{PreventivePlanExtension, PreventivePlanExtensionEntityMapper};
use entity::preventive_plan_extension_entity;
use entity::prelude::PreventivePlanExtensionEntity as ExtensionQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter, QueryOrder};

/// `HRMS-710` (`D-09`): every read goes through `tenant_select`.
pub struct PreventivePlanExtensionGateway {
    db: DbConn,
}

impl PreventivePlanExtensionGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    /// `TRM-317`: a plan's full extension history, oldest first.
    pub async fn find_by_plan(
        &self,
        plan_id: i64,
    ) -> Result<Vec<preventive_plan_extension_entity::Model>, DbErr> {
        tenant_select(ExtensionQuery::find(), preventive_plan_extension_entity::Column::TenantId)
            .filter(preventive_plan_extension_entity::Column::PreventivePlanId.eq(plan_id))
            .order_by_asc(preventive_plan_extension_entity::Column::Id)
            .all(&self.db)
            .await
    }

    /// `TRM-311`: whether a work-order item resolved by extension rather than
    /// by a real service.
    pub async fn find_by_item(
        &self,
        item_id: i64,
    ) -> Result<Option<preventive_plan_extension_entity::Model>, DbErr> {
        tenant_select(ExtensionQuery::find(), preventive_plan_extension_entity::Column::TenantId)
            .filter(preventive_plan_extension_entity::Column::WorkOrderItemId.eq(item_id))
            .one(&self.db)
            .await
    }
}

#[async_trait]
impl Gateway<PreventivePlanExtension, preventive_plan_extension_entity::Model, preventive_plan_extension_entity::ActiveModel>
    for PreventivePlanExtensionGateway
{
    async fn persist(
        &self,
        entity: PreventivePlanExtension,
    ) -> Result<preventive_plan_extension_entity::ActiveModel, DbErr> {
        PreventivePlanExtensionEntityMapper::build_active_model(entity).save(&self.db).await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            ExtensionQuery::delete_many().filter(preventive_plan_extension_entity::Column::Id.eq(id)),
            preventive_plan_extension_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<preventive_plan_extension_entity::Model>, DbErr> {
        tenant_select(ExtensionQuery::find(), preventive_plan_extension_entity::Column::TenantId)
            .filter(preventive_plan_extension_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<preventive_plan_extension_entity::Model>, DbErr> {
        tenant_select(ExtensionQuery::find(), preventive_plan_extension_entity::Column::TenantId)
            .filter(preventive_plan_extension_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<preventive_plan_extension_entity::Model>, DbErr> {
        tenant_select(ExtensionQuery::find(), preventive_plan_extension_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}
