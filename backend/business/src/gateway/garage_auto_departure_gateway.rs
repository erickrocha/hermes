use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::tenant_select;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::{GarageAttendanceStatus, GarageServiceState};
use crate::domain::garage_attendance::{GarageAttendance, GarageAttendanceEntityMapper};
use crate::domain::garage_service::{GarageService, GarageServiceEntityMapper};
use crate::domain::garage_service_log::{GarageServiceLog, GarageServiceLogEntityMapper};
use entity::{garage_attendance_entity, garage_service_entity};
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
};

#[async_trait]
pub trait GarageAutoDepartureStore: Send + Sync {
    async fn find_active_with_services(
        &self,
    ) -> Result<Vec<(GarageAttendance, Vec<GarageService>)>, BusinessError>;
    async fn close_automatically(
        &self,
        attendance: GarageAttendance,
        services: Vec<GarageService>,
    ) -> Result<bool, BusinessError>;
}

pub struct GarageAutoDepartureDatabaseGateway {
    db: DbConn,
}

impl GarageAutoDepartureDatabaseGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl GarageAutoDepartureStore for GarageAutoDepartureDatabaseGateway {
    async fn find_active_with_services(
        &self,
    ) -> Result<Vec<(GarageAttendance, Vec<GarageService>)>, BusinessError> {
        let attendances = tenant_select(
            garage_attendance_entity::Entity::find(),
            garage_attendance_entity::Column::TenantId,
        )
        .filter(garage_attendance_entity::Column::ActiveMarker.is_not_null())
        .all(&self.db)
        .await
        .map_err(database_error)?;
        let mut candidates = Vec::with_capacity(attendances.len());
        for model in attendances {
            let services = tenant_select(
                garage_service_entity::Entity::find(),
                garage_service_entity::Column::TenantId,
            )
            .filter(garage_service_entity::Column::AttendanceId.eq(model.id))
            .order_by_asc(garage_service_entity::Column::Id)
            .all(&self.db)
            .await
            .map(GarageServiceEntityMapper::from_models)
            .map_err(database_error)?;
            candidates.push((GarageAttendanceEntityMapper::from_model(model), services));
        }
        Ok(candidates)
    }

    async fn close_automatically(
        &self,
        attendance: GarageAttendance,
        services: Vec<GarageService>,
    ) -> Result<bool, BusinessError> {
        let attendance_id = attendance.id.unwrap_or_default();
        let released_at = attendance.released_at.unwrap_or_default();
        let transaction = self.db.begin().await.map_err(database_error)?;
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
        if current.status != GarageAttendanceStatus::Open.to_string()
            || current.active_marker.is_none()
        {
            return Ok(false);
        }

        for service in services {
            if service.attendance_id != attendance_id
                || service.state != GarageServiceState::NotDone
            {
                return Err(BusinessError::new(
                    "Invalid automatic departure service update".to_string(),
                ));
            }
            let service_id = service.id.unwrap_or_default();
            let current_service = tenant_select(
                garage_service_entity::Entity::find(),
                garage_service_entity::Column::TenantId,
            )
            .filter(garage_service_entity::Column::Id.eq(service_id))
            .filter(garage_service_entity::Column::AttendanceId.eq(attendance_id))
            .lock_exclusive()
            .one(&transaction)
            .await
            .map_err(database_error)?
            .ok_or_else(|| BusinessError::new("Garage service not found".to_string()))?;
            let mut active: garage_service_entity::ActiveModel = current_service.clone().into();
            active.state = Set(GarageServiceState::NotDone.to_string());
            active.marked_at = Set(Some(released_at.and_utc()));
            active.update(&transaction).await.map_err(database_error)?;

            GarageServiceLogEntityMapper::build_active_model(GarageServiceLog {
                id: None,
                uuid: None,
                tenant_id: current_service.tenant_id,
                attendance_id,
                vehicle_id: current.vehicle_id,
                service_model_id: current_service.service_model_id,
                name_key: current_service.name_key,
                new_state: GarageServiceState::NotDone,
                acted_by_user_id: None,
                acted_at: released_at,
                origin: "System: automatic departure".to_string(),
                created_at: None,
                created_by: None,
                updated_at: None,
                updated_by: None,
            })
            .save(&transaction)
            .await
            .map_err(database_error)?;
        }

        let mut updated: garage_attendance_entity::ActiveModel = current.into();
        updated.status = Set(GarageAttendanceStatus::LeftForOperation.to_string());
        updated.released_at = Set(Some(released_at.and_utc()));
        updated.manual_priority = Set(None);
        updated.automatic_departure_justification =
            Set(attendance.automatic_departure_justification);
        updated.active_marker = Set(None);
        updated.update(&transaction).await.map_err(database_error)?;
        transaction.commit().await.map_err(database_error)?;
        Ok(true)
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Garage automatic departure database error: {error}");
    log::error!("[GarageAutoDepartureGateway] {message}");
    BusinessError::new(message)
}
