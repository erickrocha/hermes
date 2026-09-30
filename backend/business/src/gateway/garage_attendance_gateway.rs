use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::garage_attendance::{GarageAttendance, GarageAttendanceEntityMapper};
use entity::prelude::GarageAttendanceEntity as GarageAttendanceQuery;
use entity::garage_attendance_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
};

/// `HRMS-958` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct GarageAttendanceGateway {
    db: DbConn,
}

impl GarageAttendanceGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<GarageAttendance, garage_attendance_entity::Model, garage_attendance_entity::ActiveModel>
    for GarageAttendanceGateway
{
    async fn persist(&self, entity: GarageAttendance) -> Result<garage_attendance_entity::ActiveModel, DbErr> {
        GarageAttendanceEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            GarageAttendanceQuery::delete_many().filter(garage_attendance_entity::Column::Id.eq(id)),
            garage_attendance_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<garage_attendance_entity::Model>, DbErr> {
        tenant_select(GarageAttendanceQuery::find(), garage_attendance_entity::Column::TenantId)
            .filter(garage_attendance_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<garage_attendance_entity::Model>, DbErr> {
        tenant_select(GarageAttendanceQuery::find(), garage_attendance_entity::Column::TenantId)
            .filter(garage_attendance_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<garage_attendance_entity::Model>, DbErr> {
        tenant_select(GarageAttendanceQuery::find(), garage_attendance_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl GarageAttendanceGateway {
    /// `TRM-412`: the vehicle's one active triage, if any.
    pub async fn find_active_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<garage_attendance_entity::Model>, DbErr> {
        tenant_select(GarageAttendanceQuery::find(), garage_attendance_entity::Column::TenantId)
            .filter(garage_attendance_entity::Column::VehicleId.eq(vehicle_id))
            .filter(garage_attendance_entity::Column::ActiveMarker.is_not_null())
            .one(&self.db)
            .await
    }

    /// Every active triage of the caller's tenant -- the yard's queue.
    pub async fn find_all_active(&self) -> Result<Vec<garage_attendance_entity::Model>, DbErr> {
        tenant_select(GarageAttendanceQuery::find(), garage_attendance_entity::Column::TenantId)
            .filter(garage_attendance_entity::Column::ActiveMarker.is_not_null())
            .all(&self.db)
            .await
    }
}
