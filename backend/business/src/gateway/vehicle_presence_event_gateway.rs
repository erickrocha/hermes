use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::business_error::BusinessError;
use crate::domain::enums::PresenceEventKind;
use crate::domain::vehicle_presence_event::{VehiclePresenceEvent, VehiclePresenceEventEntityMapper};
use entity::prelude::VehiclePresenceEventEntity as VehiclePresenceEventQuery;
use entity::vehicle_presence_event_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

#[async_trait]
pub trait VehiclePresenceStore: Send + Sync {
    async fn get_latest(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<VehiclePresenceEvent>, BusinessError>;
    async fn get_latest_of_kind(
        &self,
        vehicle_id: i64,
        kind: PresenceEventKind,
    ) -> Result<Option<VehiclePresenceEvent>, BusinessError>;
    async fn save(&self, event: VehiclePresenceEvent) -> Result<(), BusinessError>;
}

#[async_trait]
pub trait GarageValidityPresenceStore: Send + Sync {
    async fn latest_of_kind(
        &self,
        vehicle_id: i64,
        kind: PresenceEventKind,
    ) -> Result<Option<VehiclePresenceEvent>, BusinessError>;
    async fn latest_of_kind_before(
        &self,
        vehicle_id: i64,
        kind: PresenceEventKind,
        before: chrono::NaiveDateTime,
    ) -> Result<Option<VehiclePresenceEvent>, BusinessError>;
}

/// `HRMS-960` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct VehiclePresenceEventGateway {
    db: DbConn,
}

impl VehiclePresenceEventGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl VehiclePresenceStore for VehiclePresenceEventGateway {
    async fn get_latest(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<VehiclePresenceEvent>, BusinessError> {
        let model = VehiclePresenceEventGateway::find_latest(self, vehicle_id)
            .await
            .map_err(database_error)?;
        Ok(model.map(VehiclePresenceEventEntityMapper::from_model))
    }

    async fn get_latest_of_kind(
        &self,
        vehicle_id: i64,
        kind: PresenceEventKind,
    ) -> Result<Option<VehiclePresenceEvent>, BusinessError> {
        let model = VehiclePresenceEventGateway::find_latest_of_kind(
            self,
            vehicle_id,
            &kind.to_string(),
        )
        .await
        .map_err(database_error)?;
        Ok(model.map(VehiclePresenceEventEntityMapper::from_model))
    }

    async fn save(&self, event: VehiclePresenceEvent) -> Result<(), BusinessError> {
        <Self as Gateway<
            VehiclePresenceEvent,
            vehicle_presence_event_entity::Model,
            vehicle_presence_event_entity::ActiveModel,
        >>::persist(self, event)
        .await
        .map_err(database_error)?;
        Ok(())
    }
}

#[async_trait]
impl GarageValidityPresenceStore for VehiclePresenceEventGateway {
    async fn latest_of_kind(
        &self,
        vehicle_id: i64,
        kind: PresenceEventKind,
    ) -> Result<Option<VehiclePresenceEvent>, BusinessError> {
        let model = VehiclePresenceEventGateway::find_latest_of_kind(
            self,
            vehicle_id,
            &kind.to_string(),
        )
        .await
        .map_err(database_error)?;
        Ok(model.map(VehiclePresenceEventEntityMapper::from_model))
    }

    async fn latest_of_kind_before(
        &self,
        vehicle_id: i64,
        kind: PresenceEventKind,
        before: chrono::NaiveDateTime,
    ) -> Result<Option<VehiclePresenceEvent>, BusinessError> {
        let model = VehiclePresenceEventGateway::find_latest_of_kind_before(
            self,
            vehicle_id,
            &kind.to_string(),
            before,
        )
        .await
        .map_err(database_error)?;
        Ok(model.map(VehiclePresenceEventEntityMapper::from_model))
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Vehicle presence database error: {error}");
    log::error!("[VehiclePresenceEventGateway] {message}");
    BusinessError::new(message)
}

#[async_trait]
impl Gateway<VehiclePresenceEvent, vehicle_presence_event_entity::Model, vehicle_presence_event_entity::ActiveModel>
    for VehiclePresenceEventGateway
{
    async fn persist(&self, entity: VehiclePresenceEvent) -> Result<vehicle_presence_event_entity::ActiveModel, DbErr> {
        VehiclePresenceEventEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            VehiclePresenceEventQuery::delete_many().filter(vehicle_presence_event_entity::Column::Id.eq(id)),
            vehicle_presence_event_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<vehicle_presence_event_entity::Model>, DbErr> {
        tenant_select(VehiclePresenceEventQuery::find(), vehicle_presence_event_entity::Column::TenantId)
            .filter(vehicle_presence_event_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<vehicle_presence_event_entity::Model>, DbErr> {
        tenant_select(VehiclePresenceEventQuery::find(), vehicle_presence_event_entity::Column::TenantId)
            .filter(vehicle_presence_event_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<vehicle_presence_event_entity::Model>, DbErr> {
        tenant_select(VehiclePresenceEventQuery::find(), vehicle_presence_event_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl VehiclePresenceEventGateway {
    /// `TRM-770`: the vehicle's latest physical event.
    pub async fn find_latest(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<vehicle_presence_event_entity::Model>, DbErr> {
        tenant_select(VehiclePresenceEventQuery::find(), vehicle_presence_event_entity::Column::TenantId)
            .filter(vehicle_presence_event_entity::Column::VehicleId.eq(vehicle_id))
            .order_by_desc(vehicle_presence_event_entity::Column::OccurredAt)
            .order_by_desc(vehicle_presence_event_entity::Column::Id)
            .one(&self.db)
            .await
    }

    /// The vehicle's latest event of one kind.
    pub async fn find_latest_of_kind(
        &self,
        vehicle_id: i64,
        kind: &str,
    ) -> Result<Option<vehicle_presence_event_entity::Model>, DbErr> {
        tenant_select(VehiclePresenceEventQuery::find(), vehicle_presence_event_entity::Column::TenantId)
            .filter(vehicle_presence_event_entity::Column::VehicleId.eq(vehicle_id))
            .filter(vehicle_presence_event_entity::Column::Kind.eq(kind))
            .order_by_desc(vehicle_presence_event_entity::Column::OccurredAt)
            .order_by_desc(vehicle_presence_event_entity::Column::Id)
            .one(&self.db)
            .await
    }

    /// The vehicle's latest event of one kind strictly before an instant -- the
    /// departure that opened the run an arrival closed (`TRM-466`).
    pub async fn find_latest_of_kind_before(
        &self,
        vehicle_id: i64,
        kind: &str,
        before: chrono::NaiveDateTime,
    ) -> Result<Option<vehicle_presence_event_entity::Model>, DbErr> {
        tenant_select(VehiclePresenceEventQuery::find(), vehicle_presence_event_entity::Column::TenantId)
            .filter(vehicle_presence_event_entity::Column::VehicleId.eq(vehicle_id))
            .filter(vehicle_presence_event_entity::Column::Kind.eq(kind))
            .filter(vehicle_presence_event_entity::Column::OccurredAt.lt(before.and_utc()))
            .order_by_desc(vehicle_presence_event_entity::Column::OccurredAt)
            .order_by_desc(vehicle_presence_event_entity::Column::Id)
            .one(&self.db)
            .await
    }
}
