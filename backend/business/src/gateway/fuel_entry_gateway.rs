use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::fuel_entry::{FuelEntry, FuelEntryEntityMapper};
use entity::fuel_entry_entity;
use entity::prelude::FuelEntryEntity as FuelEntryQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

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
            .filter(fuel_entry_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<fuel_entry_entity::Model>, DbErr> {
        tenant_select(FuelEntryQuery::find(), fuel_entry_entity::Column::TenantId)
            .filter(fuel_entry_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<fuel_entry_entity::Model>, DbErr> {
        tenant_select(FuelEntryQuery::find(), fuel_entry_entity::Column::TenantId)
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
            .order_by_desc(fuel_entry_entity::Column::RecordedAt);
        fetch_page(query, &self.db, page, page_size).await
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
