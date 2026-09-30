use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::garage_service_log::{GarageServiceLog, GarageServiceLogEntityMapper};
use entity::prelude::GarageServiceLogEntity as GarageServiceLogQuery;
use entity::garage_service_log_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

/// `HRMS-958` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct GarageServiceLogGateway {
    db: DbConn,
}

impl GarageServiceLogGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<GarageServiceLog, garage_service_log_entity::Model, garage_service_log_entity::ActiveModel>
    for GarageServiceLogGateway
{
    async fn persist(&self, entity: GarageServiceLog) -> Result<garage_service_log_entity::ActiveModel, DbErr> {
        GarageServiceLogEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            GarageServiceLogQuery::delete_many().filter(garage_service_log_entity::Column::Id.eq(id)),
            garage_service_log_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<garage_service_log_entity::Model>, DbErr> {
        tenant_select(GarageServiceLogQuery::find(), garage_service_log_entity::Column::TenantId)
            .filter(garage_service_log_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<garage_service_log_entity::Model>, DbErr> {
        tenant_select(GarageServiceLogQuery::find(), garage_service_log_entity::Column::TenantId)
            .filter(garage_service_log_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<garage_service_log_entity::Model>, DbErr> {
        tenant_select(GarageServiceLogQuery::find(), garage_service_log_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl GarageServiceLogGateway {
    /// A triage's marking history, oldest first.
    pub async fn find_by_attendance(
        &self,
        attendance_id: i64,
    ) -> Result<Vec<garage_service_log_entity::Model>, DbErr> {
        tenant_select(GarageServiceLogQuery::find(), garage_service_log_entity::Column::TenantId)
            .filter(garage_service_log_entity::Column::AttendanceId.eq(attendance_id))
            .order_by_asc(garage_service_log_entity::Column::Id)
            .all(&self.db)
            .await
    }
}
