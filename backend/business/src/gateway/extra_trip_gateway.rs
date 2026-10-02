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
    QueryFilter, QueryOrder, SqlErr, TransactionTrait,
};

pub enum ExtraTripStoreError {
    Duplicate,
    Storage(BusinessError),
}

#[async_trait]
pub trait ExtraTripStore: Send + Sync {
    async fn save(&self, trip: ExtraTrip) -> Result<ExtraTrip, ExtraTripStoreError>;
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
        ExtraTripEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
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
        Ok(ExtraTripEntityMapper::from_models(models))
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
                }
                None => created += 1,
            }
            if let Err(error) = ExtraTripEntityMapper::build_active_model(trip)
                .save(&transaction)
                .await
            {
                let _ = transaction.rollback().await;
                return Err(database_error(error));
            }
        }
        transaction.commit().await.map_err(database_error)?;
        Ok((created, updated))
    }
}

/// `HRMS-607` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::extra_trip_entity;
    use sea_orm::{DbBackend, EntityTrait, QueryTrait};

    fn caller(tenant_id: Option<i64>, enforce_tenant: bool) -> AuditUser {
        AuditUser {
            id: 1,
            email: "owner@example.com".to_string(),
            tenant_id,
            enforce_tenant,
        }
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
}
