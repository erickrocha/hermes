use business::domain::enums::{
    GarageAttendanceOrigin, GarageAttendanceStatus, KmOrigin, WorkOrderOrigin, WorkOrderStatus,
};
use business::domain::enums::{GarageServiceState, WorkOrderItemStatus};
use business::domain::garage_attendance::GarageAttendance;
use business::domain::garage_release::{GarageRelease, GarageReleaseItem};
use business::domain::garage_service::GarageService;
use business::domain::km_evolution::KmEvolution;
use business::domain::work_order::WorkOrder;
use business::domain::work_order_item::WorkOrderItem;
use business::gateway::garage_release_gateway::GarageReleaseStore;
use business::use_cases::garage_release_use_case::work_order_outcome;
use business::use_cases::garage_release_use_case::{
    GarageReleaseCommand, GarageReleaseUseCase, RELEASE_ATTENDANCE_CLOSED,
    RELEASE_INVALID_AWAITING_SERVICE, RELEASE_JUSTIFICATION_REQUIRED, RELEASE_ODOMETER_REQUIRED,
};
use chrono::{NaiveDate, NaiveDateTime};
use std::sync::{Arc, Mutex};

type CommitCapture = Arc<Mutex<Option<(WorkOrder, Vec<WorkOrderItem>, Option<KmEvolution>)>>>;

#[test]
fn release_maps_garage_states_to_existing_work_order_item_statuses() {
    assert_eq!(
        work_order_outcome(GarageServiceState::Performed, false),
        WorkOrderItemStatus::Resolved
    );
    assert_eq!(
        work_order_outcome(GarageServiceState::NotNeeded, false),
        WorkOrderItemStatus::Cancelled
    );
    assert_eq!(
        work_order_outcome(GarageServiceState::NotDone, false),
        WorkOrderItemStatus::Pending
    );
    assert_eq!(
        work_order_outcome(GarageServiceState::Pending, false),
        WorkOrderItemStatus::Pending
    );
}

#[test]
fn release_can_explicitly_leave_a_pendency_awaiting_parts() {
    assert_eq!(
        work_order_outcome(GarageServiceState::Pending, true),
        WorkOrderItemStatus::AwaitingParts
    );
}

struct FakeStore {
    attendance: GarageAttendance,
    services: Vec<GarageService>,
    latest_odometer: Option<f64>,
    active_work_order: Option<WorkOrder>,
    existing_items: Vec<WorkOrderItem>,
    committed: CommitCapture,
}

#[sea_orm::prelude::async_trait::async_trait]
impl GarageReleaseStore for FakeStore {
    async fn find_attendance(
        &self,
        _uuid: String,
    ) -> Result<Option<GarageAttendance>, business::domain::business_error::BusinessError> {
        Ok(Some(self.attendance.clone()))
    }

    async fn list_services(
        &self,
        _attendance_id: i64,
    ) -> Result<Vec<GarageService>, business::domain::business_error::BusinessError> {
        Ok(self.services.clone())
    }

    async fn find_service_display_name(
        &self,
        _service_model_id: i64,
    ) -> Result<Option<String>, business::domain::business_error::BusinessError> {
        Ok(Some("Brake service".to_string()))
    }

    async fn latest_odometer(
        &self,
        _vehicle_id: i64,
    ) -> Result<Option<f64>, business::domain::business_error::BusinessError> {
        Ok(self.latest_odometer)
    }

    async fn find_active_work_order(
        &self,
        _vehicle_id: i64,
    ) -> Result<Option<WorkOrder>, business::domain::business_error::BusinessError> {
        Ok(self.active_work_order.clone())
    }

    async fn list_work_order_items(
        &self,
        _work_order_id: i64,
    ) -> Result<Vec<WorkOrderItem>, business::domain::business_error::BusinessError> {
        Ok(self.existing_items.clone())
    }

    async fn commit_release(
        &self,
        attendance: GarageAttendance,
        work_order: WorkOrder,
        items: Vec<WorkOrderItem>,
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
        business::domain::business_error::BusinessError,
    > {
        *self.committed.lock().unwrap() = Some((work_order.clone(), items.clone(), reading));
        Ok((
            GarageAttendance {
                status: GarageAttendanceStatus::ReleasedWithPendency,
                ..attendance
            },
            work_order,
            items,
            release,
            release_items,
        ))
    }
}

fn release_fixture(
    latest_odometer: Option<f64>,
    active_work_order: Option<WorkOrder>,
) -> (FakeStore, CommitCapture) {
    let date = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
    let now = NaiveDateTime::new(date, chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap());
    let committed = Arc::new(Mutex::new(None));
    (
        FakeStore {
            attendance: GarageAttendance {
                id: Some(21),
                uuid: Some("attendance-uuid".to_string()),
                tenant_id: Some(7),
                vehicle_id: 33,
                attendance_date: date,
                checked_in_at: now,
                status: GarageAttendanceStatus::Open,
                manual_priority: Some(1),
                released_at: None,
                automatic_departure_justification: None,
                origin: GarageAttendanceOrigin::Manual,
                active_marker: Some(1),
                created_at: None,
                created_by: None,
                updated_at: None,
                updated_by: None,
            },
            services: vec![GarageService {
                id: Some(40),
                uuid: Some("service-pending".to_string()),
                tenant_id: Some(7),
                attendance_id: 21,
                service_model_id: 12,
                name_key: "BRAKES".to_string(),
                state: GarageServiceState::Pending,
                performed_at: None,
                marked_at: None,
                forced_pending_at: None,
                created_at: None,
                created_by: None,
                updated_at: None,
                updated_by: None,
            }],
            latest_odometer,
            active_work_order,
            existing_items: Vec::new(),
            committed: committed.clone(),
        },
        committed,
    )
}

fn command(odometer_km: Option<f64>) -> GarageReleaseCommand {
    GarageReleaseCommand {
        attendance_uuid: "attendance-uuid".to_string(),
        justification: "  Part unavailable  ".to_string(),
        odometer_km,
        awaiting_parts_service_uuids: vec!["service-pending".to_string()],
        responsible_user_id: 8,
    }
}

#[tokio::test]
async fn release_requires_odometer_when_no_official_reading_exists() {
    let (store, committed) = release_fixture(None, None);
    let result = GarageReleaseUseCase::new(store)
        .release(command(None))
        .await;

    assert_eq!(result.unwrap_err().message, RELEASE_ODOMETER_REQUIRED);
    assert!(committed.lock().unwrap().is_none());
}

#[tokio::test]
async fn release_records_awaiting_parts_and_official_reading_for_new_work_order() {
    let (store, committed) = release_fixture(None, None);
    let result = GarageReleaseUseCase::new(store)
        .release(command(Some(123.0)))
        .await
        .unwrap();
    let (work_order, items, reading) = committed.lock().unwrap().clone().unwrap();

    assert_eq!(result.release.justification, "Part unavailable");
    assert_eq!(
        result.attendance.status,
        GarageAttendanceStatus::ReleasedWithPendency
    );
    assert_eq!(work_order.odometer_km, 123.0);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].status, WorkOrderItemStatus::AwaitingParts);
    assert_eq!(items[0].description, "Brake service");
    assert_eq!(reading.unwrap().origin, KmOrigin::WorkOrder);
    assert_eq!(result.release_items[0].service_name, "Brake service");
    assert_eq!(
        result.release_items[0].work_order_status,
        WorkOrderItemStatus::AwaitingParts
    );
}

#[tokio::test]
async fn release_reuses_an_active_work_order_and_latest_official_odometer() {
    let (mut store, committed) = release_fixture(Some(456.0), Some(work_order_fixture()));
    store.existing_items = Vec::new();
    let result = GarageReleaseUseCase::new(store)
        .release(command(None))
        .await
        .unwrap();
    let (work_order, _, reading) = committed.lock().unwrap().clone().unwrap();

    assert_eq!(work_order.id, Some(77));
    assert_eq!(work_order.odometer_km, 400.0);
    assert!(reading.is_none());
    assert_eq!(result.work_order.id, Some(77));
}

#[tokio::test]
async fn release_rejects_invalid_commands_and_attendance_service_states() {
    let (store, _) = release_fixture(None, None);
    let mut invalid = command(Some(123.0));
    invalid.justification = "  ".into();
    assert_eq!(
        GarageReleaseUseCase::new(store)
            .release(invalid)
            .await
            .expect_err("justification is required")
            .message,
        RELEASE_JUSTIFICATION_REQUIRED
    );

    let (store, _) = release_fixture(None, None);
    assert_eq!(
        GarageReleaseUseCase::new(store)
            .release(command(Some(-1.0)))
            .await
            .expect_err("negative odometer is invalid")
            .message,
        "Odometer reading cannot be negative"
    );

    let (mut store, _) = release_fixture(Some(100.0), None);
    store.attendance.status = GarageAttendanceStatus::Finished;
    assert_eq!(
        GarageReleaseUseCase::new(store)
            .release(command(None))
            .await
            .expect_err("closed attendance cannot be released")
            .message,
        RELEASE_ATTENDANCE_CLOSED
    );

    let (mut store, _) = release_fixture(Some(100.0), None);
    store.services.clear();
    assert_eq!(
        GarageReleaseUseCase::new(store)
            .release(command(None))
            .await
            .expect_err("attendance without services cannot be released")
            .message,
        "The triage has no services to release"
    );

    let (mut store, _) = release_fixture(Some(100.0), None);
    store.services[0].state = GarageServiceState::Performed;
    assert_eq!(
        GarageReleaseUseCase::new(store)
            .release(command(None))
            .await
            .expect_err("release requires an outstanding pendency")
            .message,
        "This release operation requires at least one garage pendency"
    );

    let (store, _) = release_fixture(Some(100.0), None);
    let mut duplicate = command(None);
    duplicate.awaiting_parts_service_uuids =
        vec!["service-pending".into(), "service-pending".into()];
    assert_eq!(
        GarageReleaseUseCase::new(store)
            .release(duplicate)
            .await
            .expect_err("duplicate service UUIDs are invalid")
            .message,
        RELEASE_INVALID_AWAITING_SERVICE
    );

    let (store, _) = release_fixture(Some(100.0), None);
    let mut unknown = command(None);
    unknown.awaiting_parts_service_uuids = vec!["unknown-service".into()];
    assert_eq!(
        GarageReleaseUseCase::new(store)
            .release(unknown)
            .await
            .expect_err("unknown service UUID is invalid")
            .message,
        RELEASE_INVALID_AWAITING_SERVICE
    );

    let (mut store, _) = release_fixture(Some(100.0), None);
    store.services[0].state = GarageServiceState::Performed;
    store.services.push(GarageService {
        id: Some(41),
        uuid: Some("another-pending".into()),
        state: GarageServiceState::Pending,
        ..store.services[0].clone()
    });
    let mut performed = command(None);
    performed.awaiting_parts_service_uuids = vec!["service-pending".into()];
    assert_eq!(
        GarageReleaseUseCase::new(store)
            .release(performed)
            .await
            .expect_err("performed services cannot await parts")
            .message,
        RELEASE_INVALID_AWAITING_SERVICE
    );
}

#[tokio::test]
async fn release_validates_tenant_and_records_reading_when_reusing_an_order() {
    let (store, committed) = release_fixture(None, Some(work_order_fixture()));
    let result = GarageReleaseUseCase::new(store)
        .release(command(Some(501.0)))
        .await
        .expect("a supplied reading can be recorded with an existing order");
    let (_, _, reading) = committed.lock().unwrap().clone().unwrap();
    assert_eq!(result.work_order.id, Some(77));
    assert_eq!(
        reading.expect("missing official reading is recorded").km,
        501.0
    );

    let (mut store, _) = release_fixture(Some(100.0), Some(work_order_fixture()));
    store.active_work_order.as_mut().unwrap().tenant_id = Some(99);
    assert_eq!(
        GarageReleaseUseCase::new(store)
            .release(command(None))
            .await
            .expect_err("a different tenant's order cannot be reused")
            .message,
        "No active work order exists for this tenant and vehicle"
    );

    let (mut store, _) = release_fixture(Some(100.0), None);
    store.attendance.id = None;
    assert_eq!(
        GarageReleaseUseCase::new(store)
            .release(command(None))
            .await
            .expect_err("attendance must have a persisted ID")
            .message,
        business::use_cases::garage_release_use_case::RELEASE_ATTENDANCE_NOT_FOUND
    );

    let (mut store, _) = release_fixture(Some(100.0), None);
    store.services[0].id = None;
    assert_eq!(
        GarageReleaseUseCase::new(store)
            .release(command(None))
            .await
            .expect_err("each garage service must have a persisted ID")
            .message,
        "Garage service has no persisted id"
    );
}

fn work_order_fixture() -> WorkOrder {
    WorkOrder {
        id: Some(77),
        uuid: Some("work-order-uuid".to_string()),
        tenant_id: Some(7),
        vehicle_id: 33,
        opened_at: None,
        odometer_km: 400.0,
        origin: WorkOrderOrigin::Manual,
        checklist_run_id: None,
        maintenance_plan_id: None,
        preventive_plan_id: None,
        service_type: None,
        description: "Existing work order".to_string(),
        responsible: None,
        status: WorkOrderStatus::Open,
        observation: None,
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
    }
}
