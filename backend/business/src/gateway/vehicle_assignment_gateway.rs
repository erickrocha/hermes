use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::vehicle_assignment::{VehicleAssignment, VehicleAssignmentEntityMapper};
use entity::prelude::VehicleAssignmentEntity as AssignmentQuery;
use entity::vehicle_assignment_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

/// EPIC-FO-03 (HRMS-933, D-09): every read and delete goes through
/// `tenant_select`/`tenant_delete`, like `VehicleGateway`.
pub struct VehicleAssignmentGateway {
    db: DbConn,
}

impl VehicleAssignmentGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<VehicleAssignment, vehicle_assignment_entity::Model, vehicle_assignment_entity::ActiveModel>
    for VehicleAssignmentGateway
{
    async fn persist(
        &self,
        entity: VehicleAssignment,
    ) -> Result<vehicle_assignment_entity::ActiveModel, DbErr> {
        VehicleAssignmentEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            AssignmentQuery::delete_many().filter(vehicle_assignment_entity::Column::Id.eq(id)),
            vehicle_assignment_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<vehicle_assignment_entity::Model>, DbErr> {
        tenant_select(AssignmentQuery::find(), vehicle_assignment_entity::Column::TenantId)
            .filter(vehicle_assignment_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<vehicle_assignment_entity::Model>, DbErr> {
        tenant_select(AssignmentQuery::find(), vehicle_assignment_entity::Column::TenantId)
            .filter(vehicle_assignment_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<vehicle_assignment_entity::Model>, DbErr> {
        tenant_select(AssignmentQuery::find(), vehicle_assignment_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl VehicleAssignmentGateway {
    /// D-23(d): the one live assignment of a vehicle, if any.
    pub async fn find_live_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<vehicle_assignment_entity::Model>, DbErr> {
        tenant_select(AssignmentQuery::find(), vehicle_assignment_entity::Column::TenantId)
            .filter(vehicle_assignment_entity::Column::VehicleId.eq(vehicle_id))
            .filter(vehicle_assignment_entity::Column::EndedAt.is_null())
            .one(&self.db)
            .await
    }

    /// PD-028: a vehicle's assignment history, newest first.
    pub async fn find_page_by_vehicle(
        &self,
        vehicle_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<vehicle_assignment_entity::Model>, u64), DbErr> {
        let query = tenant_select(AssignmentQuery::find(), vehicle_assignment_entity::Column::TenantId)
            .filter(vehicle_assignment_entity::Column::VehicleId.eq(vehicle_id))
            .order_by_desc(vehicle_assignment_entity::Column::StartedAt)
            .order_by_desc(vehicle_assignment_entity::Column::Id);
        fetch_page(query, &self.db, page, page_size).await
    }
}
