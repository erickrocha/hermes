use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::garage_service::{GarageService, GarageServiceEntityMapper};
use entity::prelude::GarageServiceEntity as GarageServiceQuery;
use entity::garage_service_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

/// `HRMS-958` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct GarageServiceGateway {
    db: DbConn,
}

impl GarageServiceGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<GarageService, garage_service_entity::Model, garage_service_entity::ActiveModel>
    for GarageServiceGateway
{
    async fn persist(&self, entity: GarageService) -> Result<garage_service_entity::ActiveModel, DbErr> {
        GarageServiceEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            GarageServiceQuery::delete_many().filter(garage_service_entity::Column::Id.eq(id)),
            garage_service_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<garage_service_entity::Model>, DbErr> {
        tenant_select(GarageServiceQuery::find(), garage_service_entity::Column::TenantId)
            .filter(garage_service_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<garage_service_entity::Model>, DbErr> {
        tenant_select(GarageServiceQuery::find(), garage_service_entity::Column::TenantId)
            .filter(garage_service_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<garage_service_entity::Model>, DbErr> {
        tenant_select(GarageServiceQuery::find(), garage_service_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl GarageServiceGateway {
    /// A triage's service records, in the order they were created.
    pub async fn find_by_attendance(
        &self,
        attendance_id: i64,
    ) -> Result<Vec<garage_service_entity::Model>, DbErr> {
        tenant_select(GarageServiceQuery::find(), garage_service_entity::Column::TenantId)
            .filter(garage_service_entity::Column::AttendanceId.eq(attendance_id))
            .order_by_asc(garage_service_entity::Column::Id)
            .all(&self.db)
            .await
    }

    /// The records of these triages that were ever performed, newest first.
    pub async fn find_performed_by_attendances(
        &self,
        attendance_ids: Vec<i64>,
    ) -> Result<Vec<garage_service_entity::Model>, DbErr> {
        tenant_select(GarageServiceQuery::find(), garage_service_entity::Column::TenantId)
            .filter(garage_service_entity::Column::AttendanceId.is_in(attendance_ids))
            .filter(garage_service_entity::Column::PerformedAt.is_not_null())
            .order_by_desc(garage_service_entity::Column::PerformedAt)
            .all(&self.db)
            .await
    }
}
