use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::business_error::BusinessError;
use crate::domain::fuel_entry::{FuelEntry, FuelEntryEntityMapper};
use entity::fuel_entry_entity;
use entity::prelude::FuelEntryEntity as FuelEntryQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

#[async_trait]
pub trait FuelEntryReadStore: Send + Sync {
    async fn has_entries(&self) -> Result<bool, BusinessError>;
    async fn reports_from(
        &self,
        from: chrono::NaiveDateTime,
    ) -> Result<Vec<FuelEntry>, BusinessError>;
}

#[async_trait]
pub trait FuelGaugeFuelStore: Send + Sync {
    async fn entries_for_vehicle(&self, vehicle_id: i64) -> Result<Vec<FuelEntry>, BusinessError>;
}

#[async_trait]
pub trait FuelEntryStore: Send + Sync {
    async fn save(&self, entry: FuelEntry) -> Result<FuelEntry, BusinessError>;
    async fn get_by_uuid(&self, uuid: String) -> Result<Option<FuelEntry>, BusinessError>;
    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<FuelEntry>, u64), BusinessError>;
    async fn find_report(
        &self,
        from: Option<chrono::NaiveDateTime>,
        to: Option<chrono::NaiveDateTime>,
        vehicle_id: Option<i64>,
        station: Option<String>,
    ) -> Result<Vec<FuelEntry>, BusinessError>;
    async fn get_last_deleted_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<FuelEntry>, BusinessError>;
}

#[async_trait]
pub trait FuelSyncStore: FuelEntryStore {
    async fn get_by_id(&self, id: i64) -> Result<Option<FuelEntry>, BusinessError>;
    async fn get_by_provider_transaction_id(
        &self,
        transaction_id: &str,
    ) -> Result<Option<FuelEntry>, BusinessError>;
    async fn find_unreconciled_driver_entries(
        &self,
        vehicle_id: i64,
        from: chrono::NaiveDateTime,
        to: chrono::NaiveDateTime,
    ) -> Result<Vec<FuelEntry>, BusinessError>;
}

/// `HRMS-942` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct FuelEntryGateway {
    db: DbConn,
}

impl FuelEntryGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl FuelEntryStore for FuelEntryGateway {
    async fn save(&self, entry: FuelEntry) -> Result<FuelEntry, BusinessError> {
        let active_model = <Self as Gateway<
            FuelEntry,
            fuel_entry_entity::Model,
            fuel_entry_entity::ActiveModel,
        >>::persist(self, entry)
        .await
        .map_err(database_error)?;
        Ok(FuelEntryEntityMapper::from_active_model(active_model))
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<FuelEntry>, BusinessError> {
        let model = <Self as Gateway<
            FuelEntry,
            fuel_entry_entity::Model,
            fuel_entry_entity::ActiveModel,
        >>::find_by_uuid(self, uuid)
        .await
        .map_err(database_error)?;
        Ok(model.map(FuelEntryEntityMapper::from_model))
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<FuelEntry>, u64), BusinessError> {
        let (models, total) = FuelEntryGateway::find_page(self, page, page_size)
            .await
            .map_err(database_error)?;
        Ok((FuelEntryEntityMapper::from_models(models), total))
    }

    async fn find_report(
        &self,
        from: Option<chrono::NaiveDateTime>,
        to: Option<chrono::NaiveDateTime>,
        vehicle_id: Option<i64>,
        station: Option<String>,
    ) -> Result<Vec<FuelEntry>, BusinessError> {
        let models = FuelEntryGateway::find_report(self, from, to, vehicle_id, station)
            .await
            .map_err(database_error)?;
        Ok(FuelEntryEntityMapper::from_models(models))
    }

    async fn get_last_deleted_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<FuelEntry>, BusinessError> {
        let model = FuelEntryGateway::find_last_deleted_by_vehicle(self, vehicle_id)
            .await
            .map_err(database_error)?;
        Ok(model.map(FuelEntryEntityMapper::from_model))
    }
}

#[async_trait]
impl FuelSyncStore for FuelEntryGateway {
    async fn get_by_id(&self, id: i64) -> Result<Option<FuelEntry>, BusinessError> {
        let model = <Self as Gateway<
            FuelEntry,
            fuel_entry_entity::Model,
            fuel_entry_entity::ActiveModel,
        >>::find_by_id(self, id)
        .await
        .map_err(database_error)?;
        Ok(model.map(FuelEntryEntityMapper::from_model))
    }

    async fn get_by_provider_transaction_id(
        &self,
        transaction_id: &str,
    ) -> Result<Option<FuelEntry>, BusinessError> {
        let model = FuelEntryGateway::find_by_provider_transaction_id(self, transaction_id)
            .await
            .map_err(database_error)?;
        Ok(model.map(FuelEntryEntityMapper::from_model))
    }

    async fn find_unreconciled_driver_entries(
        &self,
        vehicle_id: i64,
        from: chrono::NaiveDateTime,
        to: chrono::NaiveDateTime,
    ) -> Result<Vec<FuelEntry>, BusinessError> {
        let models = FuelEntryGateway::find_unreconciled_driver_entries(self, vehicle_id, from, to)
            .await
            .map_err(database_error)?;
        Ok(FuelEntryEntityMapper::from_models(models))
    }
}

#[async_trait]
impl FuelEntryReadStore for FuelEntryGateway {
    async fn has_entries(&self) -> Result<bool, BusinessError> {
        self.find_page(0, 1)
            .await
            .map(|(_, total)| total > 0)
            .map_err(database_error)
    }

    async fn reports_from(
        &self,
        from: chrono::NaiveDateTime,
    ) -> Result<Vec<FuelEntry>, BusinessError> {
        self.find_report(Some(from), None, None, None)
            .await
            .map(FuelEntryEntityMapper::from_models)
            .map_err(database_error)
    }
}

#[async_trait]
impl FuelGaugeFuelStore for FuelEntryGateway {
    async fn entries_for_vehicle(&self, vehicle_id: i64) -> Result<Vec<FuelEntry>, BusinessError> {
        self.find_report(None, None, Some(vehicle_id), None)
            .await
            .map(FuelEntryEntityMapper::from_models)
            .map_err(database_error)
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Fuel entry database error: {error}");
    log::error!("[FuelEntryGateway] {message}");
    BusinessError::new(message)
}

#[async_trait]
impl Gateway<FuelEntry, fuel_entry_entity::Model, fuel_entry_entity::ActiveModel> for FuelEntryGateway {
    async fn persist(&self, entity: FuelEntry) -> Result<fuel_entry_entity::ActiveModel, DbErr> {
        FuelEntryEntityMapper::build_active_model(entity).save(&self.db).await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            FuelEntryQuery::delete_many().filter(fuel_entry_entity::Column::Id.eq(id)),
            fuel_entry_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<fuel_entry_entity::Model>, DbErr> {
        tenant_select(FuelEntryQuery::find(), fuel_entry_entity::Column::TenantId)
            .filter(fuel_entry_entity::Column::DeletedAt.is_null())
            .filter(fuel_entry_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<fuel_entry_entity::Model>, DbErr> {
        tenant_select(FuelEntryQuery::find(), fuel_entry_entity::Column::TenantId)
            .filter(fuel_entry_entity::Column::DeletedAt.is_null())
            .filter(fuel_entry_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<fuel_entry_entity::Model>, DbErr> {
        tenant_select(FuelEntryQuery::find(), fuel_entry_entity::Column::TenantId)
            .filter(fuel_entry_entity::Column::DeletedAt.is_null())
            .all(&self.db)
            .await
    }
}

impl FuelEntryGateway {
    /// `PD-028`: every fuel entry of the caller's tenant, most recent first.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<fuel_entry_entity::Model>, u64), DbErr> {
        let query = tenant_select(FuelEntryQuery::find(), fuel_entry_entity::Column::TenantId)
            .filter(fuel_entry_entity::Column::DeletedAt.is_null())
            .order_by_desc(fuel_entry_entity::Column::RecordedAt);
        fetch_page(query, &self.db, page, page_size).await
    }

    /// `TRM-1553`: the report's rows -- the caller's tenant's fuellings inside
    /// the optional period/vehicle/station filters, oldest first, unpaged.
    pub async fn find_report(
        &self,
        from: Option<chrono::NaiveDateTime>,
        to: Option<chrono::NaiveDateTime>,
        vehicle_id: Option<i64>,
        station: Option<String>,
    ) -> Result<Vec<fuel_entry_entity::Model>, DbErr> {
        let mut query = tenant_select(FuelEntryQuery::find(), fuel_entry_entity::Column::TenantId)
            .filter(fuel_entry_entity::Column::DeletedAt.is_null())
            .order_by_asc(fuel_entry_entity::Column::RecordedAt);
        if let Some(from) = from {
            query = query.filter(fuel_entry_entity::Column::RecordedAt.gte(from.and_utc()));
        }
        if let Some(to) = to {
            query = query.filter(fuel_entry_entity::Column::RecordedAt.lte(to.and_utc()));
        }
        if let Some(vehicle_id) = vehicle_id {
            query = query.filter(fuel_entry_entity::Column::VehicleId.eq(vehicle_id));
        }
        if let Some(station) = station {
            query = query.filter(fuel_entry_entity::Column::Station.eq(station));
        }
        query.all(&self.db).await
    }

    /// `TRM-545`/`546`: driver-reported fuellings of a vehicle inside a window
    /// that no provider transaction has been matched to yet.
    pub async fn find_unreconciled_driver_entries(
        &self,
        vehicle_id: i64,
        from: chrono::NaiveDateTime,
        to: chrono::NaiveDateTime,
    ) -> Result<Vec<fuel_entry_entity::Model>, DbErr> {
        tenant_select(FuelEntryQuery::find(), fuel_entry_entity::Column::TenantId)
            .filter(fuel_entry_entity::Column::VehicleId.eq(vehicle_id))
            .filter(fuel_entry_entity::Column::DeletedAt.is_null())
            .filter(fuel_entry_entity::Column::Origin.eq("DriverPhoto"))
            .filter(fuel_entry_entity::Column::ProviderTransactionId.is_null())
            .filter(fuel_entry_entity::Column::RecordedAt.gte(from.and_utc()))
            .filter(fuel_entry_entity::Column::RecordedAt.lte(to.and_utc()))
            .all(&self.db)
            .await
    }

    /// `TRM-553`: the vehicle's most recently soft-deleted fuelling -- not one
    /// absorbed by a unification, whose litres the keeper already carries.
    pub async fn find_last_deleted_by_vehicle(&self, vehicle_id: i64) -> Result<Option<fuel_entry_entity::Model>, DbErr> {
        tenant_select(FuelEntryQuery::find(), fuel_entry_entity::Column::TenantId)
            .filter(fuel_entry_entity::Column::VehicleId.eq(vehicle_id))
            .filter(fuel_entry_entity::Column::DeletedAt.is_not_null())
            .filter(fuel_entry_entity::Column::UnifiedIntoId.is_null())
            .order_by_desc(fuel_entry_entity::Column::DeletedAt)
            .order_by_desc(fuel_entry_entity::Column::Id)
            .one(&self.db)
            .await
    }

    /// `TRM-1520`: the vehicle's latest **provider-reported** full tank that still
    /// stands (not deleted, not absorbed by a unification).
    pub async fn find_latest_provider_full_tank(&self, vehicle_id: i64) -> Result<Option<fuel_entry_entity::Model>, DbErr> {
        tenant_select(FuelEntryQuery::find(), fuel_entry_entity::Column::TenantId)
            .filter(fuel_entry_entity::Column::VehicleId.eq(vehicle_id))
            .filter(fuel_entry_entity::Column::FullTank.eq(true))
            .filter(fuel_entry_entity::Column::Origin.eq("CtaSync"))
            .filter(fuel_entry_entity::Column::DeletedAt.is_null())
            .order_by_desc(fuel_entry_entity::Column::RecordedAt)
            .order_by_desc(fuel_entry_entity::Column::Id)
            .one(&self.db)
            .await
    }

    /// `TRM-511`/`512`: identifies an already-imported transaction so the
    /// sync can correct it in place rather than re-import it as a duplicate.
    pub async fn find_by_provider_transaction_id(
        &self,
        provider_transaction_id: &str,
    ) -> Result<Option<fuel_entry_entity::Model>, DbErr> {
        tenant_select(FuelEntryQuery::find(), fuel_entry_entity::Column::TenantId)
            .filter(fuel_entry_entity::Column::ProviderTransactionId.eq(provider_transaction_id))
            .one(&self.db)
            .await
    }
}

/// `HRMS-942` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::fuel_entry_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_fuel_entries() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(fuel_entry_entity::Entity::find(), fuel_entry_entity::Column::TenantId)
                .build(DbBackend::MySql)
                .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_fuel_entry_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(fuel_entry_entity::Entity::find(), fuel_entry_entity::Column::TenantId)
                .build(DbBackend::MySql)
                .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_fuel_entry_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                fuel_entry_entity::Entity::delete_many(),
                fuel_entry_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
