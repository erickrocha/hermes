use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{tenant_delete, tenant_select};
use crate::domain::business_error::BusinessError;
use crate::domain::enums::GarageAttendanceStatus;
use crate::domain::garage_attendance::{GarageAttendance, GarageAttendanceEntityMapper};
use crate::domain::garage_release::{
    GarageRelease, GarageReleaseEntityMapper, GarageReleaseItem, GarageReleaseItemEntityMapper,
};
use crate::domain::garage_service::{GarageService, GarageServiceEntityMapper};
use crate::domain::km_evolution::{KmEvolution, KmEvolutionEntityMapper};
use crate::domain::work_order::{WorkOrder, WorkOrderEntityMapper};
use crate::domain::work_order_item::{WorkOrderItem, WorkOrderItemEntityMapper};
use entity::{
    garage_attendance_entity, garage_service_entity, garage_service_model_entity,
    km_evolution_entity, vehicle_entity, work_order_entity, work_order_item_entity,
};
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, TransactionTrait,
};

#[async_trait]
pub trait GarageReleaseStore: Send + Sync {
    async fn find_attendance(
        &self,
        uuid: String,
    ) -> Result<Option<GarageAttendance>, BusinessError>;
    async fn list_services(&self, attendance_id: i64) -> Result<Vec<GarageService>, BusinessError>;
    async fn find_service_display_name(
        &self,
        service_model_id: i64,
    ) -> Result<Option<String>, BusinessError>;
    async fn latest_odometer(&self, vehicle_id: i64) -> Result<Option<f64>, BusinessError>;
    async fn find_active_work_order(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<WorkOrder>, BusinessError>;
    async fn list_work_order_items(
        &self,
        work_order_id: i64,
    ) -> Result<Vec<WorkOrderItem>, BusinessError>;
    async fn commit_release(
        &self,
        attendance: GarageAttendance,
        work_order: WorkOrder,
        work_order_items: Vec<WorkOrderItem>,
        reading: Option<KmEvolution>,
        release: GarageRelease,
        release_items: Vec<GarageReleaseItem>,
    ) -> Result<
        (
            GarageAttendance,
            WorkOrder,
            Vec<WorkOrderItem>,
            GarageRelease,
            Vec<GarageReleaseItem>,
        ),
        BusinessError,
    >;
}

pub struct GarageReleaseDatabaseGateway {
    db: DbConn,
}

impl GarageReleaseDatabaseGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    pub async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, BusinessError> {
        tenant_delete(
            entity::garage_release_entity::Entity::delete_many()
                .filter(entity::garage_release_entity::Column::Id.eq(id)),
            entity::garage_release_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
        .map_err(database_error)
    }
}

#[async_trait]
impl GarageReleaseStore for GarageReleaseDatabaseGateway {
    async fn find_attendance(
        &self,
        uuid: String,
    ) -> Result<Option<GarageAttendance>, BusinessError> {
        tenant_select(
            garage_attendance_entity::Entity::find(),
            garage_attendance_entity::Column::TenantId,
        )
        .filter(garage_attendance_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
        .one(&self.db)
        .await
        .map(|model| model.map(GarageAttendanceEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn list_services(&self, attendance_id: i64) -> Result<Vec<GarageService>, BusinessError> {
        tenant_select(
            garage_service_entity::Entity::find(),
            garage_service_entity::Column::TenantId,
        )
        .filter(garage_service_entity::Column::AttendanceId.eq(attendance_id))
        .order_by_asc(garage_service_entity::Column::Id)
        .all(&self.db)
        .await
        .map(GarageServiceEntityMapper::from_models)
        .map_err(database_error)
    }

    async fn find_service_display_name(
        &self,
        service_model_id: i64,
    ) -> Result<Option<String>, BusinessError> {
        tenant_select(
            garage_service_model_entity::Entity::find(),
            garage_service_model_entity::Column::TenantId,
        )
        .filter(garage_service_model_entity::Column::Id.eq(service_model_id))
        .one(&self.db)
        .await
        .map(|model| model.map(|service| service.name))
        .map_err(database_error)
    }

    async fn latest_odometer(&self, vehicle_id: i64) -> Result<Option<f64>, BusinessError> {
        tenant_select(
            km_evolution_entity::Entity::find(),
            km_evolution_entity::Column::TenantId,
        )
        .filter(km_evolution_entity::Column::VehicleId.eq(vehicle_id))
        .order_by_desc(km_evolution_entity::Column::RecordedAt)
        .order_by_desc(km_evolution_entity::Column::Id)
        .one(&self.db)
        .await
        .map(|model| model.map(|reading| reading.km))
        .map_err(database_error)
    }

    async fn find_active_work_order(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<WorkOrder>, BusinessError> {
        tenant_select(
            work_order_entity::Entity::find(),
            work_order_entity::Column::TenantId,
        )
        .filter(work_order_entity::Column::VehicleId.eq(vehicle_id))
        .filter(work_order_entity::Column::Status.is_not_in(["Concluded", "Cancelled"]))
        .order_by_asc(work_order_entity::Column::Id)
        .one(&self.db)
        .await
        .map(|model| model.map(WorkOrderEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn list_work_order_items(
        &self,
        work_order_id: i64,
    ) -> Result<Vec<WorkOrderItem>, BusinessError> {
        tenant_select(
            work_order_item_entity::Entity::find(),
            work_order_item_entity::Column::TenantId,
        )
        .filter(work_order_item_entity::Column::WorkOrderId.eq(work_order_id))
        .order_by_asc(work_order_item_entity::Column::Id)
        .all(&self.db)
        .await
        .map(WorkOrderItemEntityMapper::from_models)
        .map_err(database_error)
    }

    async fn commit_release(
        &self,
        attendance: GarageAttendance,
        work_order: WorkOrder,
        work_order_items: Vec<WorkOrderItem>,
        mut reading: Option<KmEvolution>,
        release: GarageRelease,
        release_items: Vec<GarageReleaseItem>,
    ) -> Result<
        (
            GarageAttendance,
            WorkOrder,
            Vec<WorkOrderItem>,
            GarageRelease,
            Vec<GarageReleaseItem>,
        ),
        BusinessError,
    > {
        if work_order_items.len() != release_items.len() {
            return Err(BusinessError::new(
                "Garage release items do not match work order items".to_string(),
            ));
        }
        let transaction = self.db.begin().await.map_err(database_error)?;
        let attendance_id = attendance.id.unwrap_or_default();
        let current = tenant_select(
            garage_attendance_entity::Entity::find(),
            garage_attendance_entity::Column::TenantId,
        )
        .filter(garage_attendance_entity::Column::Id.eq(attendance_id))
        .lock_exclusive()
        .one(&transaction)
        .await
        .map_err(database_error)?
        .ok_or_else(|| BusinessError::new("Triage not found".to_string()))?;
        if current.status != GarageAttendanceStatus::Open.to_string() {
            return Err(BusinessError::new(
                "This triage is already closed".to_string(),
            ));
        }

        let saved_order = WorkOrderEntityMapper::build_active_model(work_order)
            .save(&transaction)
            .await
            .map_err(database_error)?;
        let work_order = WorkOrderEntityMapper::from_active_model(saved_order);
        let work_order_id = work_order.id.unwrap_or_default();

        if let Some(mut odometer_reading) = reading.take() {
            odometer_reading.source_entity_id = Some(work_order_id);
            KmEvolutionEntityMapper::build_active_model(odometer_reading)
                .save(&transaction)
                .await
                .map_err(database_error)?;

            let latest = tenant_select(
                km_evolution_entity::Entity::find(),
                km_evolution_entity::Column::TenantId,
            )
            .filter(km_evolution_entity::Column::VehicleId.eq(work_order.vehicle_id))
            .order_by_desc(km_evolution_entity::Column::RecordedAt)
            .order_by_desc(km_evolution_entity::Column::Id)
            .one(&transaction)
            .await
            .map_err(database_error)?;
            if let Some(latest) = latest
                && let Some(vehicle) = tenant_select(
                    vehicle_entity::Entity::find(),
                    vehicle_entity::Column::TenantId,
                )
                .filter(vehicle_entity::Column::Id.eq(work_order.vehicle_id))
                .one(&transaction)
                .await
                .map_err(database_error)?
            {
                let mut vehicle: vehicle_entity::ActiveModel = vehicle.into();
                vehicle.odometer_km = sea_orm::Set(Some(latest.km));
                vehicle.update(&transaction).await.map_err(database_error)?;
            }
        }

        let mut saved_work_order_items = Vec::with_capacity(work_order_items.len());
        for item in work_order_items {
            let item = WorkOrderItem {
                work_order_id,
                tenant_id: work_order.tenant_id,
                ..item
            };
            let saved = WorkOrderItemEntityMapper::build_active_model(item)
                .save(&transaction)
                .await
                .map_err(database_error)?;
            saved_work_order_items.push(WorkOrderItemEntityMapper::from_active_model(saved));
        }

        let release = GarageRelease {
            work_order_id,
            tenant_id: work_order.tenant_id,
            ..release
        };
        let saved_release = GarageReleaseEntityMapper::build_active_model(release)
            .save(&transaction)
            .await
            .map_err(database_error)?;
        let release = GarageReleaseEntityMapper::from_active_model(saved_release);
        let release_id = release.id.unwrap_or_default();

        let mut saved_release_items = Vec::with_capacity(release_items.len());
        for (item, work_order_item) in release_items.into_iter().zip(&saved_work_order_items) {
            let linked = GarageReleaseItem {
                garage_release_id: release_id,
                work_order_item_id: work_order_item.id.unwrap_or_default(),
                tenant_id: work_order.tenant_id,
                ..item
            };
            let saved = GarageReleaseItemEntityMapper::build_active_model(linked)
                .save(&transaction)
                .await
                .map_err(database_error)?;
            saved_release_items.push(GarageReleaseItemEntityMapper::from_active_model(saved));
        }

        let attendance = GarageAttendanceEntityMapper::from_model(current);
        let updated_attendance = GarageAttendance {
            status: GarageAttendanceStatus::ReleasedWithPendency,
            released_at: Some(chrono::Utc::now().naive_utc()),
            active_marker: None,
            manual_priority: None,
            ..attendance
        };
        let saved_attendance = GarageAttendanceEntityMapper::build_active_model(updated_attendance)
            .save(&transaction)
            .await
            .map_err(database_error)?;
        let attendance = GarageAttendanceEntityMapper::from_active_model(saved_attendance);

        transaction.commit().await.map_err(database_error)?;
        Ok((
            attendance,
            work_order,
            saved_work_order_items,
            release,
            saved_release_items,
        ))
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Garage release database error: {error}");
    log::error!("[GarageReleaseGateway] {message}");
    BusinessError::new(message)
}
