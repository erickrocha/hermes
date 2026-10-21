use business::domain::business_error::BusinessError;
use business::domain::daily_schedule::DailySchedule;
use business::domain::enums::{DemandKind, ScheduleExceptionType, TripStatus};
use business::domain::extra_trip::ExtraTrip;
use business::domain::schedule_exception::ScheduleException;
use business::domain::transport_demand::TransportDemand;
use business::domain::transport_demand_allocation::TransportDemandAllocation;
use business::gateway::effective_schedule_gateway::EffectiveScheduleStore;
use business::use_cases::effective_schedule::ItemKind;
use business::use_cases::effective_schedule_use_case::EffectiveScheduleUseCase;
use chrono::{NaiveDate, NaiveTime};
use sea_orm::prelude::async_trait::async_trait;

struct MemoryScheduleStore {
    demands: Vec<TransportDemand>,
    allocations: Vec<TransportDemandAllocation>,
    entries: Vec<DailySchedule>,
    exceptions: Vec<ScheduleException>,
    trips: Vec<ExtraTrip>,
    fail_demands: bool,
}

#[async_trait]
impl EffectiveScheduleStore for MemoryScheduleStore {
    async fn demands(&self) -> Result<Vec<TransportDemand>, BusinessError> {
        if self.fail_demands {
            return Err(BusinessError::new("demand lookup failed".into()));
        }
        Ok(self.demands.clone())
    }

    async fn allocations(&self) -> Result<Vec<TransportDemandAllocation>, BusinessError> {
        Ok(self.allocations.clone())
    }

    async fn daily_entries(&self, date: NaiveDate) -> Result<Vec<DailySchedule>, BusinessError> {
        Ok(self
            .entries
            .iter()
            .filter(|entry| entry.date == date)
            .cloned()
            .collect())
    }

    async fn exceptions(&self, date: NaiveDate) -> Result<Vec<ScheduleException>, BusinessError> {
        Ok(self
            .exceptions
            .iter()
            .filter(|exception| exception.date == date)
            .cloned()
            .collect())
    }

    async fn trips(&self, date: NaiveDate) -> Result<Vec<ExtraTrip>, BusinessError> {
        Ok(self
            .trips
            .iter()
            .filter(|trip| trip.trip_date == date)
            .cloned()
            .collect())
    }

    async fn is_holiday(&self, _date: NaiveDate) -> Result<bool, BusinessError> {
        Ok(false)
    }

    async fn day_off_customers(&self, _date: NaiveDate) -> Result<Vec<i64>, BusinessError> {
        Ok(vec![700])
    }
}

fn trip(id: i64, date: NaiveDate, start_hour: u32, vehicle_id: Option<i64>) -> ExtraTrip {
    ExtraTrip {
        id: Some(id),
        uuid: None,
        tenant_id: Some(42),
        order_code: format!("trip-{id}"),
        trip_date: date,
        customer_id: Some(700),
        start_time: Some(NaiveTime::from_hms_opt(start_hour, 0, 0).unwrap()),
        return_date: None,
        return_time: None,
        destination: None,
        origin_city: None,
        stops: None,
        preferred_vehicle_type: None,
        driver_id: Some(5),
        second_driver_id: None,
        vehicle_id,
        freight_value_cents: None,
        payment: None,
        status: TripStatus::Scheduled,
        origin: None,
        import_batch_id: None,
        imported_at: None,
        replaced_by_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn store(date: NaiveDate) -> MemoryScheduleStore {
    MemoryScheduleStore {
        demands: vec![TransportDemand {
            id: Some(1),
            uuid: None,
            tenant_id: Some(42),
            demand_type: "RecurringLine".into(),
            demand_kind: Some(DemandKind::Line),
            customer_id: Some(701),
            line_name: Some("Morning route".into()),
            shift_start: Some(NaiveTime::from_hms_opt(8, 0, 0).unwrap()),
            shift_end: Some(NaiveTime::from_hms_opt(9, 0, 0).unwrap()),
            days_of_week: Some("Wed".into()),
            specific_date: None,
            priority: Some(1),
            preferred_vehicle_type: None,
            preferred_vehicle_model: None,
            specific_driver_id: None,
            specific_vehicle_id: None,
            active: true,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }],
        allocations: vec![TransportDemandAllocation {
            id: Some(2),
            uuid: None,
            tenant_id: Some(42),
            demand_id: 1,
            driver_id: 5,
            vehicle_id: 33,
            days_of_week: None,
            start_date: date,
            end_date: None,
            active: true,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }],
        entries: vec![DailySchedule {
            id: Some(3),
            uuid: None,
            tenant_id: Some(42),
            demand_id: 99,
            driver_id: 6,
            vehicle_id: 34,
            date,
            start_time: Some(NaiveTime::from_hms_opt(7, 0, 0).unwrap()),
            end_time: Some(NaiveTime::from_hms_opt(8, 0, 0).unwrap()),
            notes: Some("Manual entry".into()),
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }],
        exceptions: vec![ScheduleException {
            id: Some(4),
            uuid: None,
            tenant_id: Some(42),
            demand_id: 1,
            date,
            exception_type: ScheduleExceptionType::Cancellation,
            new_driver_id: None,
            new_vehicle_id: None,
            reason: Some("Holiday".into()),
            extra_trip_id: None,
            status: Some("Active".into()),
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }],
        trips: vec![
            trip(10, date, 10, Some(33)),
            trip(11, date, 8, Some(33)),
            trip(12, date, 7, None),
        ],
        fail_demands: false,
    }
}

#[tokio::test]
async fn day_and_departures_use_all_inputs_and_return_vehicle_trips_in_time_order() {
    let date = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
    let use_case = EffectiveScheduleUseCase::new(store(date));

    let day = use_case.day(date).await.unwrap();
    assert!(
        day.items
            .iter()
            .any(|item| item.kind == ItemKind::ExtraTrip)
    );

    let departures = use_case
        .departures_by_vehicle(date, date + chrono::Duration::days(1))
        .await
        .unwrap();
    let vehicle = departures.get(&33).unwrap();
    assert_eq!(vehicle.len(), 2);
    assert!(vehicle[0].at < vehicle[1].at);
    assert!(vehicle.iter().all(|departure| departure.is_trip));
    assert!(!departures.contains_key(&34));
}

#[tokio::test]
async fn schedule_store_errors_are_propagated() {
    let date = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
    let mut store = store(date);
    store.fail_demands = true;

    let error = EffectiveScheduleUseCase::new(store)
        .day(date)
        .await
        .unwrap_err();
    assert_eq!(error.message, "demand lookup failed");
}
