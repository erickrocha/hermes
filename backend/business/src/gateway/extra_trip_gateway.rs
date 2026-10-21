use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::business_error::BusinessError;
use crate::domain::extra_trip::{ExtraTrip, ExtraTripEntityMapper};
use entity::extra_trip_entity;
use entity::prelude::ExtraTripEntity as ExtraTripQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DbConn, DbErr, DeleteResult, EntityTrait,
    QueryFilter, QueryOrder, Set, SqlErr, TransactionTrait,
};

pub enum ExtraTripStoreError {
    Duplicate,
    Storage(BusinessError),
}

#[async_trait]
pub trait ExtraTripStore: Send + Sync {
    async fn save(&self, trip: ExtraTrip) -> Result<ExtraTrip, ExtraTripStoreError>;
    async fn authorize_preparation_skip(
        &self,
        id: i64,
        reason: String,
        actor: String,
        authorized_at: chrono::NaiveDateTime,
    ) -> Result<(), BusinessError>;
    async fn find_by_id(&self, id: i64) -> Result<Option<ExtraTrip>, BusinessError>;
    async fn find_by_uuid(&self, uuid: String) -> Result<Option<ExtraTrip>, BusinessError>;
    async fn find_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<ExtraTrip>, u64), BusinessError>;
    async fn find_by_code_and_date(
        &self,
        order_code: &str,
        trip_date: sea_orm::prelude::Date,
        tenant_id: Option<i64>,
    ) -> Result<Option<ExtraTrip>, BusinessError>;
    async fn import(&self, trips: Vec<ExtraTrip>) -> Result<(u64, u64), BusinessError>;
}

#[async_trait]
pub trait GarageValidityTripStore: Send + Sync {
    async fn upcoming_for_vehicle(
        &self,
        vehicle_id: i64,
        from: chrono::NaiveDate,
    ) -> Result<Vec<ExtraTrip>, BusinessError>;
}

/// `HRMS-607` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct ExtraTripGateway {
    db: DbConn,
}

impl ExtraTripGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    /// `EPIC-SC-03-S02`: the import use case needs the raw connection to
    /// open a transaction, the same way `CityGateway::db` serves
    /// `CityUseCase::import`.
    pub fn db(&self) -> &DbConn {
        &self.db
    }
}

#[async_trait]
impl Gateway<ExtraTrip, extra_trip_entity::Model, extra_trip_entity::ActiveModel>
    for ExtraTripGateway
{
    async fn persist(&self, entity: ExtraTrip) -> Result<extra_trip_entity::ActiveModel, DbErr> {
        let mut active = ExtraTripEntityMapper::build_active_model(entity.clone());
        if let Some(id) = entity.id {
            let existing = <Self as Gateway<
                ExtraTrip,
                extra_trip_entity::Model,
                extra_trip_entity::ActiveModel,
            >>::find_by_id(self, id)
            .await?;
            if existing
                .as_ref()
                .is_some_and(|existing| !same_preparation_snapshot(existing, &entity))
            {
                clear_preparation_skip(&mut active);
            }
        }
        active.save(&self.db).await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            ExtraTripQuery::delete_many().filter(extra_trip_entity::Column::Id.eq(id)),
            extra_trip_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<extra_trip_entity::Model>, DbErr> {
        tenant_select(ExtraTripQuery::find(), extra_trip_entity::Column::TenantId)
            .filter(extra_trip_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<extra_trip_entity::Model>, DbErr> {
        tenant_select(ExtraTripQuery::find(), extra_trip_entity::Column::TenantId)
            .filter(extra_trip_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<extra_trip_entity::Model>, DbErr> {
        tenant_select(ExtraTripQuery::find(), extra_trip_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl ExtraTripGateway {
    /// `TRM-784`: the vehicle's trips from `from` on that are still to be made,
    /// newest registration first -- when the latest entered the system marks
    /// when its preparation must be redone.
    pub async fn find_upcoming_by_vehicle(
        &self,
        vehicle_id: i64,
        from: chrono::NaiveDate,
    ) -> Result<Vec<extra_trip_entity::Model>, DbErr> {
        tenant_select(ExtraTripQuery::find(), extra_trip_entity::Column::TenantId)
            .filter(extra_trip_entity::Column::VehicleId.eq(vehicle_id))
            .filter(extra_trip_entity::Column::TripDate.gte(from))
            .filter(extra_trip_entity::Column::Status.ne("Cancelled"))
            .order_by_desc(extra_trip_entity::Column::CreatedAt)
            .all(&self.db)
            .await
    }

    async fn authorize_preparation_skip(
        &self,
        id: i64,
        reason: String,
        actor: String,
        authorized_at: chrono::NaiveDateTime,
    ) -> Result<(), BusinessError> {
        let trip = <Self as Gateway<
            ExtraTrip,
            extra_trip_entity::Model,
            extra_trip_entity::ActiveModel,
        >>::find_by_id(self, id)
        .await
        .map_err(database_error)?
        .ok_or_else(|| BusinessError::new("Trip not found".to_string()))?;
        let mut active = ExtraTripEntityMapper::build_active_model(
            ExtraTripEntityMapper::from_model(trip.clone()),
        );
        active.id = Set(trip.id);
        active.preparation_skip_authorized_by = Set(Some(actor));
        active.preparation_skip_authorized_at = Set(Some(authorized_at.and_utc()));
        active.preparation_skip_justification = Set(Some(reason));
        active.preparation_skip_vehicle_id = Set(trip.vehicle_id);
        active.preparation_skip_trip_date = Set(Some(trip.trip_date));
        active.preparation_skip_start_time = Set(trip.start_time);
        active.preparation_skip_return_date = Set(trip.return_date);
        active.preparation_skip_return_time = Set(trip.return_time);
        active.preparation_skip_destination = Set(trip.destination);
        active.update(&self.db).await.map_err(database_error)?;
        Ok(())
    }

    /// `PD-028`.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<extra_trip_entity::Model>, u64), DbErr> {
        let mut query = tenant_select(ExtraTripQuery::find(), extra_trip_entity::Column::TenantId)
            .order_by_desc(extra_trip_entity::Column::TripDate)
            .order_by_desc(extra_trip_entity::Column::Id);
        if let Some(term) = search {
            query = query
                .filter(Condition::any().add(extra_trip_entity::Column::OrderCode.contains(term)));
        }
        fetch_page(query, &self.db, page, page_size).await
    }

    /// `C-031` (`HRMS-860`): a driver's own trips -- the ones naming them as the driver or the second driver.
    pub async fn find_page_for_driver(
        &self,
        user_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<extra_trip_entity::Model>, u64), DbErr> {
        let query = tenant_select(ExtraTripQuery::find(), extra_trip_entity::Column::TenantId)
            .filter(
                Condition::any()
                    .add(extra_trip_entity::Column::DriverId.eq(user_id))
                    .add(extra_trip_entity::Column::SecondDriverId.eq(user_id)),
            )
            .order_by_desc(extra_trip_entity::Column::TripDate)
            .order_by_desc(extra_trip_entity::Column::Id);
        fetch_page(query, &self.db, page, page_size).await
    }

    /// `D-24(d)`/`HRMS-607`: the duplicate check `uq_extra_trip_tenant_
    /// code_date` makes a reportable error rather than a raw constraint
    /// violation -- tenant-scoped for the same `DEF-FO-01` reason
    /// `vehicle_gateway::find_by_plate` states.
    pub async fn find_by_code_and_date(
        &self,
        order_code: &str,
        trip_date: sea_orm::prelude::Date,
        tenant_id: Option<i64>,
    ) -> Result<Option<extra_trip_entity::Model>, DbErr> {
        code_date_query(order_code, trip_date, tenant_id)
            .one(&self.db)
            .await
    }
}

fn code_date_query(
    order_code: &str,
    trip_date: sea_orm::prelude::Date,
    tenant_id: Option<i64>,
) -> sea_orm::Select<ExtraTripQuery> {
    tenant_select(ExtraTripQuery::find(), extra_trip_entity::Column::TenantId)
        .filter(extra_trip_entity::Column::OrderCode.eq(order_code))
        .filter(extra_trip_entity::Column::TripDate.eq(trip_date))
        .filter(extra_trip_entity::Column::TenantId.eq(tenant_id))
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Extra trip database error: {error}");
    log::error!("[ExtraTripGateway] {message}");
    BusinessError::new(message)
}

impl ExtraTripGateway {
    /// `TRM-001`: the non-cancelled trips under way on a date -- from their trip
    /// date to their return date (or the trip date alone when none is given).
    pub async fn find_on_date(
        &self,
        date: chrono::NaiveDate,
    ) -> Result<Vec<extra_trip_entity::Model>, DbErr> {
        let trips = tenant_select(ExtraTripQuery::find(), extra_trip_entity::Column::TenantId)
            .filter(extra_trip_entity::Column::TripDate.lte(date))
            .filter(extra_trip_entity::Column::Status.ne("Cancelled"))
            .all(&self.db)
            .await?;
        Ok(trips
            .into_iter()
            .filter(|t| t.return_date.unwrap_or(t.trip_date) >= date)
            .collect())
    }
}

#[async_trait]
impl GarageValidityTripStore for ExtraTripGateway {
    async fn upcoming_for_vehicle(
        &self,
        vehicle_id: i64,
        from: chrono::NaiveDate,
    ) -> Result<Vec<ExtraTrip>, BusinessError> {
        let models = ExtraTripGateway::find_upcoming_by_vehicle(self, vehicle_id, from)
            .await
            .map_err(database_error)?;
        Ok(ExtraTripEntityMapper::from_models(
            trips_without_valid_preparation_skip(models),
        ))
    }
}

#[async_trait]
impl ExtraTripStore for ExtraTripGateway {
    async fn save(&self, trip: ExtraTrip) -> Result<ExtraTrip, ExtraTripStoreError> {
        let active = <Self as Gateway<
            ExtraTrip,
            extra_trip_entity::Model,
            extra_trip_entity::ActiveModel,
        >>::persist(self, trip)
        .await
        .map_err(|error| match error.sql_err() {
            Some(SqlErr::UniqueConstraintViolation(_)) => ExtraTripStoreError::Duplicate,
            _ => ExtraTripStoreError::Storage(database_error(error)),
        })?;
        Ok(ExtraTripEntityMapper::from_active_model(active))
    }

    async fn authorize_preparation_skip(
        &self,
        id: i64,
        reason: String,
        actor: String,
        authorized_at: chrono::NaiveDateTime,
    ) -> Result<(), BusinessError> {
        ExtraTripGateway::authorize_preparation_skip(self, id, reason, actor, authorized_at).await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<ExtraTrip>, BusinessError> {
        <Self as Gateway<
            ExtraTrip,
            extra_trip_entity::Model,
            extra_trip_entity::ActiveModel,
        >>::find_by_id(self, id)
        .await
        .map(|model| model.map(ExtraTripEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<ExtraTrip>, BusinessError> {
        <Self as Gateway<
            ExtraTrip,
            extra_trip_entity::Model,
            extra_trip_entity::ActiveModel,
        >>::find_by_uuid(self, uuid)
        .await
        .map(|model| model.map(ExtraTripEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn find_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<ExtraTrip>, u64), BusinessError> {
        ExtraTripGateway::find_page(self, page, page_size, search)
            .await
            .map(|(models, total)| (ExtraTripEntityMapper::from_models(models), total))
            .map_err(database_error)
    }

    async fn find_by_code_and_date(
        &self,
        order_code: &str,
        trip_date: sea_orm::prelude::Date,
        tenant_id: Option<i64>,
    ) -> Result<Option<ExtraTrip>, BusinessError> {
        ExtraTripGateway::find_by_code_and_date(self, order_code, trip_date, tenant_id)
            .await
            .map(|model| model.map(ExtraTripEntityMapper::from_model))
            .map_err(database_error)
    }

    async fn import(&self, trips: Vec<ExtraTrip>) -> Result<(u64, u64), BusinessError> {
        let transaction = self.db.begin().await.map_err(database_error)?;
        let mut created = 0_u64;
        let mut updated = 0_u64;
        for mut trip in trips {
            let existing = match code_date_query(&trip.order_code, trip.trip_date, trip.tenant_id)
                .one(&transaction)
                .await
            {
                Ok(existing) => existing,
                Err(error) => {
                    let _ = transaction.rollback().await;
                    return Err(database_error(error));
                }
            };
            match existing {
                Some(existing) => {
                    trip.id = Some(existing.id);
                    updated += 1;
                    let mut active = ExtraTripEntityMapper::build_active_model(trip.clone());
                    if !same_preparation_snapshot(&existing, &trip) {
                        clear_preparation_skip(&mut active);
                    }
                    if let Err(error) = active.save(&transaction).await {
                        let _ = transaction.rollback().await;
                        return Err(database_error(error));
                    }
                }
                None => {
                    created += 1;
                    if let Err(error) = ExtraTripEntityMapper::build_active_model(trip)
                        .save(&transaction)
                        .await
                    {
                        let _ = transaction.rollback().await;
                        return Err(database_error(error));
                    }
                }
            }
        }
        transaction.commit().await.map_err(database_error)?;
        Ok((created, updated))
    }
}

fn same_preparation_snapshot(existing: &extra_trip_entity::Model, next: &ExtraTrip) -> bool {
    existing.vehicle_id == next.vehicle_id
        && existing.trip_date == next.trip_date
        && existing.start_time == next.start_time
        && existing.return_date == next.return_date
        && existing.return_time == next.return_time
        && existing.destination == next.destination
}

fn clear_preparation_skip(active: &mut extra_trip_entity::ActiveModel) {
    active.preparation_skip_authorized_by = Set(None);
    active.preparation_skip_authorized_at = Set(None);
    active.preparation_skip_justification = Set(None);
    active.preparation_skip_vehicle_id = Set(None);
    active.preparation_skip_trip_date = Set(None);
    active.preparation_skip_start_time = Set(None);
    active.preparation_skip_return_date = Set(None);
    active.preparation_skip_return_time = Set(None);
    active.preparation_skip_destination = Set(None);
}

fn preparation_skip_matches(trip: &extra_trip_entity::Model) -> bool {
    trip.preparation_skip_authorized_by
        .as_ref()
        .is_some_and(|actor| !actor.trim().is_empty())
        && trip
            .preparation_skip_justification
            .as_ref()
            .is_some_and(|reason| !reason.trim().is_empty())
        && trip.preparation_skip_authorized_at.is_some()
        && trip.preparation_skip_vehicle_id == trip.vehicle_id
        && trip.preparation_skip_trip_date == Some(trip.trip_date)
        && trip.preparation_skip_start_time == trip.start_time
        && trip.preparation_skip_return_date == trip.return_date
        && trip.preparation_skip_return_time == trip.return_time
        && trip.preparation_skip_destination == trip.destination
}

fn trips_without_valid_preparation_skip(
    trips: Vec<extra_trip_entity::Model>,
) -> Vec<extra_trip_entity::Model> {
    trips
        .into_iter()
        .filter(|trip| !preparation_skip_matches(trip))
        .collect()
}

/// `HRMS-607` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::entity_mapper::EntityMapper;
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use chrono::{NaiveDate, NaiveTime};
    use entity::audit::{AuditUser, run_with_user};
    use entity::extra_trip_entity;
    use sea_orm::{
        DatabaseBackend, DbBackend, EntityTrait, MockDatabase, MockExecResult, QueryTrait, Value,
    };
    use std::collections::BTreeMap;

    fn authorized_trip() -> extra_trip_entity::Model {
        let date = NaiveDate::from_ymd_opt(2026, 10, 15).expect("valid test date");
        let start = NaiveTime::from_hms_opt(8, 0, 0).expect("valid test time");
        extra_trip_entity::Model {
            id: 10,
            uuid: vec![10],
            tenant_id: Some(42),
            order_code: "TRIP-10".into(),
            trip_date: date,
            customer_id: None,
            start_time: Some(start),
            return_date: Some(date),
            return_time: Some(NaiveTime::from_hms_opt(12, 0, 0).expect("valid test time")),
            destination: Some("Terminal".into()),
            origin_city: None,
            stops: None,
            preferred_vehicle_type: None,
            driver_id: None,
            second_driver_id: None,
            vehicle_id: Some(7),
            freight_value_cents: None,
            payment: None,
            status: "Scheduled".into(),
            origin: None,
            import_batch_id: None,
            imported_at: None,
            replaced_by_id: None,
            created_at: chrono::Utc::now(),
            created_by: None,
            updated_at: chrono::Utc::now(),
            updated_by: None,
            preparation_skip_authorized_by: Some("owner@example.com".into()),
            preparation_skip_authorized_at: Some(chrono::Utc::now()),
            preparation_skip_justification: Some("Vehicle is already prepared".into()),
            preparation_skip_vehicle_id: Some(7),
            preparation_skip_trip_date: Some(date),
            preparation_skip_start_time: Some(start),
            preparation_skip_return_date: Some(date),
            preparation_skip_return_time: Some(
                NaiveTime::from_hms_opt(12, 0, 0).expect("valid test time"),
            ),
            preparation_skip_destination: Some("Terminal".into()),
        }
    }

    #[test]
    fn a_skip_authorization_only_matches_its_complete_trip_snapshot() {
        let trip = authorized_trip();
        assert!(super::preparation_skip_matches(&trip));

        let mut changed = trip.clone();
        changed.vehicle_id = Some(8);
        assert!(!super::preparation_skip_matches(&changed));
        let mut changed = trip.clone();
        changed.trip_date = changed.trip_date.succ_opt().expect("valid next date");
        assert!(!super::preparation_skip_matches(&changed));
        let mut changed = trip.clone();
        changed.start_time = None;
        assert!(!super::preparation_skip_matches(&changed));
        let mut changed = trip.clone();
        changed.return_date = None;
        assert!(!super::preparation_skip_matches(&changed));
        let mut changed = trip.clone();
        changed.return_time = None;
        assert!(!super::preparation_skip_matches(&changed));
        let mut changed = trip;
        changed.destination = Some("Different terminal".into());
        assert!(!super::preparation_skip_matches(&changed));
    }

    #[test]
    fn an_authorized_trip_does_not_hide_another_trip_for_the_same_vehicle() {
        let authorized = authorized_trip();
        let mut other = authorized.clone();
        other.id = 11;
        other.order_code = "TRIP-11".into();
        other.preparation_skip_authorized_by = None;
        other.preparation_skip_authorized_at = None;
        other.preparation_skip_justification = None;
        other.preparation_skip_vehicle_id = None;
        other.preparation_skip_trip_date = None;
        other.preparation_skip_start_time = None;
        other.preparation_skip_return_date = None;
        other.preparation_skip_return_time = None;
        other.preparation_skip_destination = None;
        let trips = super::trips_without_valid_preparation_skip(vec![authorized, other]);

        assert_eq!(trips.len(), 1);
        assert_eq!(trips[0].id, 11);
    }

    fn caller(tenant_id: Option<i64>, enforce_tenant: bool) -> AuditUser {
        AuditUser {
            id: 1,
            email: "owner@example.com".to_string(),
            tenant_id,
            enforce_tenant,
        }
    }

    fn count(value: i64) -> Vec<Vec<BTreeMap<&'static str, Value>>> {
        vec![vec![BTreeMap::from([(
            "num_items",
            Value::BigInt(Some(value)),
        )])]]
    }

    #[tokio::test]
    async fn a_tenant_bound_caller_reads_only_its_own_trips() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                extra_trip_entity::Entity::find(),
                extra_trip_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_trip_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                extra_trip_entity::Entity::find(),
                extra_trip_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_trip_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                extra_trip_entity::Entity::delete_many(),
                extra_trip_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }

    #[tokio::test]
    async fn the_code_and_date_check_of_an_unrestricted_caller_names_the_owning_tenant() {
        // DEF-FO-01's lesson, applied here: the SysAdmin's scope adds no
        // filter, so the tenant must.
        let sql = run_with_user(Some(caller(None, false)), async {
            super::code_date_query(
                "ORD-1",
                chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                Some(99),
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("`tenant_id` = 99"), "{sql}");
    }

    #[tokio::test]
    async fn upcoming_trips_exclude_only_valid_preparation_skips() {
        let authorized = authorized_trip();
        let mut ordinary = authorized.clone();
        ordinary.id = 11;
        ordinary.order_code = "TRIP-11".into();
        ordinary.preparation_skip_authorized_by = None;
        ordinary.preparation_skip_authorized_at = None;
        ordinary.preparation_skip_justification = None;
        ordinary.preparation_skip_vehicle_id = None;
        ordinary.preparation_skip_trip_date = None;
        ordinary.preparation_skip_start_time = None;
        ordinary.preparation_skip_return_date = None;
        ordinary.preparation_skip_return_time = None;
        ordinary.preparation_skip_destination = None;
        let db = MockDatabase::new(DatabaseBackend::MySql)
            .append_query_results([[authorized, ordinary]])
            .into_connection();
        let gateway = super::ExtraTripGateway::new(db);
        let from = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();

        let trips = run_with_user(
            Some(caller(Some(42), true)),
            super::GarageValidityTripStore::upcoming_for_vehicle(&gateway, 7, from),
        )
        .await
        .unwrap();

        assert_eq!(trips.len(), 1);
        assert_eq!(trips[0].id, Some(11));
    }

    #[tokio::test]
    async fn trip_pages_support_search_and_driver_filtering() {
        let trip = authorized_trip();
        let db = MockDatabase::new(DatabaseBackend::MySql)
            .append_query_results(count(1))
            .append_query_results([[trip.clone()]])
            .append_query_results(count(2))
            .append_query_results([[trip]])
            .into_connection();
        let gateway = super::ExtraTripGateway::new(db);

        let (searched, searched_total) = run_with_user(
            Some(caller(Some(42), true)),
            gateway.find_page(0, 10, Some("TRIP-10")),
        )
        .await
        .unwrap();
        let (driver_trips, driver_total) = run_with_user(
            Some(caller(Some(42), true)),
            gateway.find_page_for_driver(5, 0, 10),
        )
        .await
        .unwrap();

        assert_eq!(searched_total, 1);
        assert_eq!(searched.len(), 1);
        assert_eq!(driver_total, 2);
        assert_eq!(driver_trips.len(), 1);
    }

    #[tokio::test]
    async fn active_on_date_keeps_trips_that_span_the_date() {
        let date = NaiveDate::from_ymd_opt(2026, 10, 16).unwrap();
        let mut spanning = authorized_trip();
        spanning.return_date = Some(date);
        let mut ended = authorized_trip();
        ended.id = 11;
        ended.trip_date = NaiveDate::from_ymd_opt(2026, 10, 14).unwrap();
        ended.return_date = Some(NaiveDate::from_ymd_opt(2026, 10, 15).unwrap());
        let db = MockDatabase::new(DatabaseBackend::MySql)
            .append_query_results([[spanning, ended]])
            .into_connection();
        let gateway = super::ExtraTripGateway::new(db);

        let trips = run_with_user(Some(caller(Some(42), true)), gateway.find_on_date(date))
            .await
            .unwrap();

        assert_eq!(trips.len(), 1);
        assert_eq!(trips[0].id, 10);
    }

    #[tokio::test]
    async fn authorizing_a_preparation_skip_stores_the_trip_snapshot() {
        let db = MockDatabase::new(DatabaseBackend::MySql)
            .append_query_results([[authorized_trip()]])
            .append_exec_results([MockExecResult {
                last_insert_id: 10,
                rows_affected: 1,
            }])
            .append_query_results([[authorized_trip()]])
            .into_connection();
        let gateway = super::ExtraTripGateway::new(db.clone());
        let authorized_at = NaiveDate::from_ymd_opt(2026, 10, 15)
            .unwrap()
            .and_hms_opt(7, 0, 0)
            .unwrap();

        run_with_user(
            Some(caller(Some(42), true)),
            super::ExtraTripStore::authorize_preparation_skip(
                &gateway,
                10,
                "vehicle already prepared".into(),
                "owner@example.com".into(),
                authorized_at,
            ),
        )
        .await
        .unwrap();

        let sql = format!("{:?}", db.into_transaction_log());
        assert!(sql.contains("vehicle already prepared"), "{sql}");
        assert!(sql.contains("owner@example.com"), "{sql}");
    }

    #[tokio::test]
    async fn import_inserts_new_trips_and_commits_the_batch() {
        let db = MockDatabase::new(DatabaseBackend::MySql)
            .append_query_results([Vec::<extra_trip_entity::Model>::new()])
            .append_exec_results([MockExecResult {
                last_insert_id: 90,
                rows_affected: 1,
            }])
            .append_query_results([[authorized_trip()]])
            .into_connection();
        let gateway = super::ExtraTripGateway::new(db);
        let mut trip =
            crate::domain::extra_trip::ExtraTripEntityMapper::from_model(authorized_trip());
        trip.id = None;
        trip.order_code = "TRIP-NEW".into();

        let result = run_with_user(
            Some(caller(Some(42), true)),
            super::ExtraTripStore::import(&gateway, vec![trip]),
        )
        .await
        .unwrap();

        assert_eq!(result, (1, 0));
    }
}
