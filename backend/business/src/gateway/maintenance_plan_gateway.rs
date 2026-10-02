use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::maintenance_plan::{MaintenancePlan, MaintenancePlanEntityMapper};
use entity::maintenance_plan_entity;
use entity::prelude::MaintenancePlanEntity as MaintenancePlanQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

/// `HRMS-703` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct MaintenancePlanGateway {
    db: DbConn,
}

impl MaintenancePlanGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<MaintenancePlan, maintenance_plan_entity::Model, maintenance_plan_entity::ActiveModel>
    for MaintenancePlanGateway
{
    async fn persist(
        &self,
        entity: MaintenancePlan,
    ) -> Result<maintenance_plan_entity::ActiveModel, DbErr> {
        MaintenancePlanEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            MaintenancePlanQuery::delete_many().filter(maintenance_plan_entity::Column::Id.eq(id)),
            maintenance_plan_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(
        &self,
        id: i64,
    ) -> Result<Option<maintenance_plan_entity::Model>, DbErr> {
        tenant_select(MaintenancePlanQuery::find(), maintenance_plan_entity::Column::TenantId)
            .filter(maintenance_plan_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<maintenance_plan_entity::Model>, DbErr> {
        tenant_select(MaintenancePlanQuery::find(), maintenance_plan_entity::Column::TenantId)
            .filter(maintenance_plan_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<maintenance_plan_entity::Model>, DbErr> {
        tenant_select(MaintenancePlanQuery::find(), maintenance_plan_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl MaintenancePlanGateway {
    /// `PD-028`: every plan of the caller's tenant, most recently created
    /// first.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<maintenance_plan_entity::Model>, u64), DbErr> {
        let query = tenant_select(MaintenancePlanQuery::find(), maintenance_plan_entity::Column::TenantId)
            .order_by_desc(maintenance_plan_entity::Column::Id);
        fetch_page(query, &self.db, page, page_size).await
    }

    /// `TRM-234`: every plan already covering this vehicle on this date,
    /// regardless of status -- the use case decides which ones are still
    /// active (`TRM-237`).
    pub async fn find_by_vehicle_and_date(
        &self,
        vehicle_id: i64,
        date: chrono::NaiveDate,
    ) -> Result<Vec<maintenance_plan_entity::Model>, DbErr> {
        tenant_select(MaintenancePlanQuery::find(), maintenance_plan_entity::Column::TenantId)
            .filter(maintenance_plan_entity::Column::VehicleId.eq(vehicle_id))
            .filter(maintenance_plan_entity::Column::Date.eq(date))
            .all(&self.db)
            .await
    }
}

/// `HRMS-703` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::maintenance_plan_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_plans() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                maintenance_plan_entity::Entity::find(),
                maintenance_plan_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_plan_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                maintenance_plan_entity::Entity::find(),
                maintenance_plan_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_plan_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                maintenance_plan_entity::Entity::delete_many(),
                maintenance_plan_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
