use business::gateway::customer_day_off_gateway::CustomerDayOffGateway;
use business::gateway::daily_schedule_gateway::DailyScheduleGateway;
use business::gateway::effective_schedule_gateway::{
    EffectiveScheduleDatabaseGateway, EffectiveScheduleStore,
};
use business::gateway::extra_trip_gateway::ExtraTripGateway;
use business::gateway::holiday_gateway::HolidayGateway;
use business::gateway::schedule_exception_gateway::ScheduleExceptionGateway;
use business::gateway::transport_demand_allocation_gateway::TransportDemandAllocationGateway;
use business::gateway::transport_demand_gateway::TransportDemandGateway;
use chrono::NaiveDate;
use entity::audit::{AuditUser, run_with_user};
use entity::{
    customer_day_off_entity, daily_schedule_entity, extra_trip_entity, holiday_entity,
    schedule_exception_entity, transport_demand_allocation_entity, transport_demand_entity,
};
use sea_orm::{DatabaseBackend, MockDatabase};

#[tokio::test]
async fn schedule_gateway_loads_all_inputs_for_a_day() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([Vec::<transport_demand_entity::Model>::new()])
        .append_query_results([Vec::<transport_demand_allocation_entity::Model>::new()])
        .append_query_results([Vec::<daily_schedule_entity::Model>::new()])
        .append_query_results([Vec::<schedule_exception_entity::Model>::new()])
        .append_query_results([Vec::<extra_trip_entity::Model>::new()])
        .append_query_results([Vec::<holiday_entity::Model>::new()])
        .append_query_results([Vec::<customer_day_off_entity::Model>::new()])
        .into_connection();
    let store = EffectiveScheduleDatabaseGateway::new(
        TransportDemandGateway::new(db.clone()),
        TransportDemandAllocationGateway::new(db.clone()),
        DailyScheduleGateway::new(db.clone()),
        ScheduleExceptionGateway::new(db.clone()),
        HolidayGateway::new(db.clone()),
        CustomerDayOffGateway::new(db.clone()),
        ExtraTripGateway::new(db),
    );
    let date = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
    let user = AuditUser {
        id: 1,
        email: "owner@example.com".into(),
        tenant_id: Some(42),
        enforce_tenant: true,
    };

    let result = run_with_user(Some(user), async {
        Ok::<_, business::domain::business_error::BusinessError>((
            store.demands().await?,
            store.allocations().await?,
            store.daily_entries(date).await?,
            store.exceptions(date).await?,
            store.trips(date).await?,
            store.is_holiday(date).await?,
            store.day_off_customers(date).await?,
        ))
    })
    .await
    .unwrap();

    assert!(result.0.is_empty());
    assert!(result.1.is_empty());
    assert!(result.2.is_empty());
    assert!(result.3.is_empty());
    assert!(result.4.is_empty());
    assert!(!result.5);
    assert!(result.6.is_empty());
}
