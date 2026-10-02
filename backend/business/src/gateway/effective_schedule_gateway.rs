use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::daily_schedule::{DailySchedule, DailyScheduleEntityMapper};
use crate::domain::extra_trip::{ExtraTrip, ExtraTripEntityMapper};
use crate::domain::schedule_exception::{ScheduleException, ScheduleExceptionEntityMapper};
use crate::domain::transport_demand::{TransportDemand, TransportDemandEntityMapper};
use crate::domain::transport_demand_allocation::{
    TransportDemandAllocation, TransportDemandAllocationEntityMapper,
};
use crate::gateway::customer_day_off_gateway::CustomerDayOffGateway;
use crate::gateway::daily_schedule_gateway::DailyScheduleGateway;
use crate::gateway::extra_trip_gateway::ExtraTripGateway;
use crate::gateway::holiday_gateway::HolidayGateway;
use crate::gateway::schedule_exception_gateway::ScheduleExceptionGateway;
use crate::gateway::transport_demand_allocation_gateway::TransportDemandAllocationGateway;
use crate::gateway::transport_demand_gateway::TransportDemandGateway;
use chrono::NaiveDate;
use sea_orm::DbErr;
use sea_orm::prelude::async_trait::async_trait;

#[async_trait]
pub trait EffectiveScheduleStore: Send + Sync {
    async fn demands(&self) -> Result<Vec<TransportDemand>, BusinessError>;
    async fn allocations(&self) -> Result<Vec<TransportDemandAllocation>, BusinessError>;
    async fn daily_entries(&self, date: NaiveDate) -> Result<Vec<DailySchedule>, BusinessError>;
    async fn exceptions(&self, date: NaiveDate) -> Result<Vec<ScheduleException>, BusinessError>;
    async fn trips(&self, date: NaiveDate) -> Result<Vec<ExtraTrip>, BusinessError>;
    async fn is_holiday(&self, date: NaiveDate) -> Result<bool, BusinessError>;
    async fn day_off_customers(&self, date: NaiveDate) -> Result<Vec<i64>, BusinessError>;
}

pub struct EffectiveScheduleDatabaseGateway {
    demands: TransportDemandGateway,
    allocations: TransportDemandAllocationGateway,
    daily: DailyScheduleGateway,
    exceptions: ScheduleExceptionGateway,
    holidays: HolidayGateway,
    day_offs: CustomerDayOffGateway,
    trips: ExtraTripGateway,
}

impl EffectiveScheduleDatabaseGateway {
    pub fn new(
        demands: TransportDemandGateway,
        allocations: TransportDemandAllocationGateway,
        daily: DailyScheduleGateway,
        exceptions: ScheduleExceptionGateway,
        holidays: HolidayGateway,
        day_offs: CustomerDayOffGateway,
        trips: ExtraTripGateway,
    ) -> Self {
        Self {
            demands,
            allocations,
            daily,
            exceptions,
            holidays,
            day_offs,
            trips,
        }
    }
}

#[async_trait]
impl EffectiveScheduleStore for EffectiveScheduleDatabaseGateway {
    async fn demands(&self) -> Result<Vec<TransportDemand>, BusinessError> {
        <TransportDemandGateway as Gateway<
            TransportDemand,
            entity::transport_demand_entity::Model,
            entity::transport_demand_entity::ActiveModel,
        >>::find_all(&self.demands)
        .await
        .map(TransportDemandEntityMapper::from_models)
        .map_err(database_error)
    }

    async fn allocations(&self) -> Result<Vec<TransportDemandAllocation>, BusinessError> {
        <TransportDemandAllocationGateway as Gateway<
            TransportDemandAllocation,
            entity::transport_demand_allocation_entity::Model,
            entity::transport_demand_allocation_entity::ActiveModel,
        >>::find_all(&self.allocations)
        .await
        .map(TransportDemandAllocationEntityMapper::from_models)
        .map_err(database_error)
    }

    async fn daily_entries(&self, date: NaiveDate) -> Result<Vec<DailySchedule>, BusinessError> {
        self.daily
            .find_by_date(date)
            .await
            .map(DailyScheduleEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn exceptions(&self, date: NaiveDate) -> Result<Vec<ScheduleException>, BusinessError> {
        self.exceptions
            .find_by_date(date)
            .await
            .map(ScheduleExceptionEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn trips(&self, date: NaiveDate) -> Result<Vec<ExtraTrip>, BusinessError> {
        self.trips
            .find_on_date(date)
            .await
            .map(ExtraTripEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn is_holiday(&self, date: NaiveDate) -> Result<bool, BusinessError> {
        self.holidays
            .find_by_date(date)
            .await
            .map(|holidays: Vec<entity::holiday_entity::Model>| !holidays.is_empty())
            .map_err(database_error)
    }

    async fn day_off_customers(&self, date: NaiveDate) -> Result<Vec<i64>, BusinessError> {
        self.day_offs
            .find_by_date(date)
            .await
            .map(|rows: Vec<entity::customer_day_off_entity::Model>| {
                rows.into_iter()
                    .map(|day_off| day_off.customer_id)
                    .collect()
            })
            .map_err(database_error)
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Database error: {error}");
    log::error!("[EffectiveScheduleGateway] {message}");
    BusinessError::new(message)
}
