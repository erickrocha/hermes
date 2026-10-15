use crate::domain::business_error::BusinessError;
use crate::domain::enums::{
    GarageAttendanceStatus, GarageServiceState, KmOrigin, WorkOrderItemStatus, WorkOrderOrigin,
    WorkOrderStatus,
};
use crate::domain::garage_attendance::GarageAttendance;
use crate::domain::garage_release::{GarageRelease, GarageReleaseItem};
use crate::domain::km_evolution::KmEvolution;
use crate::domain::work_order::WorkOrder;
use crate::domain::work_order_item::WorkOrderItem;
use crate::gateway::garage_release_gateway::GarageReleaseStore;
use crate::use_cases::work_order_use_case::WorkOrderUseCase;
use chrono::Utc;
use std::collections::HashSet;

pub const RELEASE_JUSTIFICATION_REQUIRED: &str = "A release justification is required";
pub const RELEASE_ATTENDANCE_NOT_FOUND: &str = "Triage not found";
pub const RELEASE_ATTENDANCE_CLOSED: &str = "This triage is already closed";
pub const RELEASE_ODOMETER_REQUIRED: &str =
    "An odometer reading is required because the vehicle has no official reading";
pub const RELEASE_INVALID_AWAITING_SERVICE: &str =
    "Only pending garage services can be marked as awaiting parts";

pub fn work_order_outcome(
    garage_state: GarageServiceState,
    awaiting_parts: bool,
) -> WorkOrderItemStatus {
    if awaiting_parts {
        return WorkOrderItemStatus::AwaitingParts;
    }

    match garage_state {
        GarageServiceState::Performed => WorkOrderItemStatus::Resolved,
        GarageServiceState::NotNeeded => WorkOrderItemStatus::Cancelled,
        GarageServiceState::Pending | GarageServiceState::NotDone => WorkOrderItemStatus::Pending,
    }
}

#[derive(Debug, Clone)]
pub struct GarageReleaseCommand {
    pub attendance_uuid: String,
    pub justification: String,
    pub odometer_km: Option<f64>,
    pub awaiting_parts_service_uuids: Vec<String>,
    pub responsible_user_id: i64,
}

#[derive(Debug, Clone)]
pub struct GarageReleaseResult {
    pub attendance: GarageAttendance,
    pub work_order: WorkOrder,
    pub work_order_items: Vec<WorkOrderItem>,
    pub release: GarageRelease,
    pub release_items: Vec<GarageReleaseItem>,
}

pub struct GarageReleaseUseCase {
    store: Box<dyn GarageReleaseStore>,
}

impl GarageReleaseUseCase {
    pub fn new<S: GarageReleaseStore + 'static>(store: S) -> Self {
        Self {
            store: Box::new(store),
        }
    }

    pub async fn release(
        &self,
        command: GarageReleaseCommand,
    ) -> Result<GarageReleaseResult, BusinessError> {
        let justification = command.justification.trim().to_string();
        if justification.is_empty() {
            return Err(BusinessError::new(
                RELEASE_JUSTIFICATION_REQUIRED.to_string(),
            ));
        }
        if command.odometer_km.is_some_and(|km| km < 0.0) {
            return Err(BusinessError::new(
                "Odometer reading cannot be negative".to_string(),
            ));
        }

        let attendance = self
            .store
            .find_attendance(command.attendance_uuid.clone())
            .await?
            .ok_or_else(|| BusinessError::new(RELEASE_ATTENDANCE_NOT_FOUND.to_string()))?;
        if attendance.status != GarageAttendanceStatus::Open {
            return Err(BusinessError::new(RELEASE_ATTENDANCE_CLOSED.to_string()));
        }
        let attendance_id = attendance
            .id
            .ok_or_else(|| BusinessError::new(RELEASE_ATTENDANCE_NOT_FOUND.to_string()))?;
        let services = self.store.list_services(attendance_id).await?;
        if services.is_empty() {
            return Err(BusinessError::new(
                "The triage has no services to release".to_string(),
            ));
        }
        if !services.iter().any(|service| {
            matches!(
                service.state,
                GarageServiceState::Pending | GarageServiceState::NotDone
            )
        }) {
            return Err(BusinessError::new(
                "This release operation requires at least one garage pendency".to_string(),
            ));
        }

        let awaiting_parts: HashSet<_> = command
            .awaiting_parts_service_uuids
            .iter()
            .cloned()
            .collect();
        if awaiting_parts.len() != command.awaiting_parts_service_uuids.len() {
            return Err(BusinessError::new(
                RELEASE_INVALID_AWAITING_SERVICE.to_string(),
            ));
        }
        for uuid in &awaiting_parts {
            let Some(service) = services
                .iter()
                .find(|service| service.uuid.as_ref() == Some(uuid))
            else {
                return Err(BusinessError::new(
                    RELEASE_INVALID_AWAITING_SERVICE.to_string(),
                ));
            };
            if !matches!(
                service.state,
                GarageServiceState::Pending | GarageServiceState::NotDone
            ) {
                return Err(BusinessError::new(
                    RELEASE_INVALID_AWAITING_SERVICE.to_string(),
                ));
            }
        }

        let latest_odometer = self.store.latest_odometer(attendance.vehicle_id).await?;
        let existing_order = self
            .store
            .find_active_work_order(attendance.vehicle_id)
            .await?;
        if existing_order
            .as_ref()
            .is_some_and(|order| order.tenant_id != attendance.tenant_id)
        {
            return Err(BusinessError::new(
                "No active work order exists for this tenant and vehicle".to_string(),
            ));
        }
        let needs_reading = existing_order.is_none() || latest_odometer.is_none();
        let odometer_km = match (latest_odometer, command.odometer_km) {
            (Some(official), _) => official,
            (None, Some(provided)) => provided,
            (None, None) => return Err(BusinessError::new(RELEASE_ODOMETER_REQUIRED.to_string())),
        };

        let now = Utc::now().naive_utc();
        let work_order = match existing_order {
            Some(order) => order,
            None => WorkOrder {
                id: None,
                uuid: None,
                tenant_id: attendance.tenant_id,
                vehicle_id: attendance.vehicle_id,
                opened_at: Some(now),
                odometer_km,
                origin: WorkOrderOrigin::Manual,
                checklist_run_id: None,
                maintenance_plan_id: None,
                preventive_plan_id: None,
                service_type: Some("Garage".to_string()),
                description: format!("Garage release {}", command.attendance_uuid),
                responsible: None,
                status: WorkOrderStatus::Open,
                observation: Some(justification.clone()),
                external_service: false,
                supplier: None,
                invoice_number: None,
                invoice_value_cents: None,
                invoice_date: None,
                concluded_at: None,
                created_at: None,
                created_by: None,
                updated_at: None,
                updated_by: None,
            },
        };
        let mut work_order_items = if let Some(id) = work_order.id {
            self.store.list_work_order_items(id).await?
        } else {
            Vec::new()
        };
        let mut new_items = Vec::with_capacity(services.len());
        let mut release_items = Vec::with_capacity(services.len());
        for service in services {
            let service_id = service.id.ok_or_else(|| {
                BusinessError::new("Garage service has no persisted id".to_string())
            })?;
            let service_name = self
                .store
                .find_service_display_name(service.service_model_id)
                .await?
                .unwrap_or_else(|| service.name_key.clone());
            let status = work_order_outcome(
                service.state,
                service
                    .uuid
                    .as_ref()
                    .is_some_and(|uuid| awaiting_parts.contains(uuid)),
            );
            let item = WorkOrderItem {
                id: None,
                uuid: None,
                tenant_id: attendance.tenant_id,
                work_order_id: work_order.id.unwrap_or_default(),
                description: service_name.clone(),
                item_type: Some("GarageService".to_string()),
                status: status.clone(),
                observation: Some(format!("Released from garage: {justification}")),
                resolved_by: (status == WorkOrderItemStatus::Resolved)
                    .then(|| command.responsible_user_id.to_string()),
                resolved_at: (status == WorkOrderItemStatus::Resolved).then_some(now),
                resolution_description: (status == WorkOrderItemStatus::Resolved)
                    .then(|| "Performed during garage attendance".to_string()),
                purchase_order_id: None,
                is_purchase_placeholder: false,
                preventive_plan_id: None,
                created_at: None,
                created_by: None,
                updated_at: None,
                updated_by: None,
            };
            release_items.push(GarageReleaseItem {
                id: None,
                uuid: None,
                tenant_id: attendance.tenant_id,
                garage_release_id: 0,
                garage_service_id: service_id,
                work_order_item_id: 0,
                service_name,
                garage_state: service.state,
                work_order_status: status,
                created_at: None,
                created_by: None,
                updated_at: None,
                updated_by: None,
            });
            new_items.push(item);
        }

        let mut status = work_order.status.clone();
        for item in &new_items {
            work_order_items.push(item.clone());
            status = WorkOrderUseCase::recompute_status(&status, item, &work_order_items);
        }
        let work_order = WorkOrder {
            status,
            ..work_order
        };
        let reading = needs_reading.then_some(KmEvolution {
            id: None,
            uuid: None,
            tenant_id: attendance.tenant_id,
            vehicle_id: attendance.vehicle_id,
            km: odometer_km,
            recorded_at: Some(now),
            origin: KmOrigin::WorkOrder,
            source_entity: Some("work_order".to_string()),
            source_entity_id: work_order.id,
            notes: Some("Odometer captured during garage release".to_string()),
            recorded_by_user_id: Some(command.responsible_user_id),
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        });
        let release = GarageRelease {
            id: None,
            uuid: None,
            tenant_id: attendance.tenant_id,
            attendance_id,
            vehicle_id: attendance.vehicle_id,
            work_order_id: work_order.id.unwrap_or_default(),
            justification,
            responsible_user_id: command.responsible_user_id,
            released_at: now,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        };

        let (attendance, work_order, work_order_items, release, release_items) = self
            .store
            .commit_release(
                attendance,
                work_order,
                new_items,
                reading,
                release,
                release_items,
            )
            .await?;
        Ok(GarageReleaseResult {
            attendance,
            work_order,
            work_order_items,
            release,
            release_items,
        })
    }
}
