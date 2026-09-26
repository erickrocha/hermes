use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::km_evolution::{KmEvolution, KmEvolutionEntityMapper};
use entity::km_evolution_entity;
use entity::prelude::KmEvolutionEntity as KmEvolutionQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

/// `HRMS-650` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct KmEvolutionGateway {
    db: DbConn,
}

impl KmEvolutionGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<KmEvolution, km_evolution_entity::Model, km_evolution_entity::ActiveModel>
    for KmEvolutionGateway
{
    async fn persist(
        &self,
        entity: KmEvolution,
    ) -> Result<km_evolution_entity::ActiveModel, DbErr> {
        KmEvolutionEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            KmEvolutionQuery::delete_many().filter(km_evolution_entity::Column::Id.eq(id)),
            km_evolution_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<km_evolution_entity::Model>, DbErr> {
        tenant_select(KmEvolutionQuery::find(), km_evolution_entity::Column::TenantId)
            .filter(km_evolution_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<km_evolution_entity::Model>, DbErr> {
        tenant_select(KmEvolutionQuery::find(), km_evolution_entity::Column::TenantId)
            .filter(km_evolution_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<km_evolution_entity::Model>, DbErr> {
        tenant_select(KmEvolutionQuery::find(), km_evolution_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl KmEvolutionGateway {
    /// `PD-028`: a vehicle's odometer history, most recently recorded first.
    pub async fn find_page_by_vehicle(
        &self,
        vehicle_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<km_evolution_entity::Model>, u64), DbErr> {
        let query = tenant_select(KmEvolutionQuery::find(), km_evolution_entity::Column::TenantId)
            .filter(km_evolution_entity::Column::VehicleId.eq(vehicle_id))
            .order_by_desc(km_evolution_entity::Column::RecordedAt)
            .order_by_desc(km_evolution_entity::Column::Id);
        fetch_page(query, &self.db, page, page_size).await
    }

    /// `AD-041`/`TRM-155`: the vehicle's official reading is whichever entry
    /// is **chronologically latest by `recorded_at`**, not whichever was
    /// inserted last -- a backdated correction (`Adjustment`) does not need
    /// a separate edit path, it just does not win this query unless it
    /// truly is the newest reading.
    pub async fn find_latest_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<km_evolution_entity::Model>, DbErr> {
        tenant_select(KmEvolutionQuery::find(), km_evolution_entity::Column::TenantId)
            .filter(km_evolution_entity::Column::VehicleId.eq(vehicle_id))
            .order_by_desc(km_evolution_entity::Column::RecordedAt)
            .order_by_desc(km_evolution_entity::Column::Id)
            .one(&self.db)
            .await
    }
}

/// `HRMS-650` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::km_evolution_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_readings() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                km_evolution_entity::Entity::find(),
                km_evolution_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_reading_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                km_evolution_entity::Entity::find(),
                km_evolution_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_reading_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                km_evolution_entity::Entity::delete_many(),
                km_evolution_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
