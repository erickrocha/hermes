use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::preventive_plan::{PreventivePlan, PreventivePlanEntityMapper};
use entity::preventive_plan_entity;
use entity::prelude::PreventivePlanEntity as PreventivePlanQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DbConn, DbErr, DeleteResult, EntityTrait,
    QueryFilter, QueryOrder,
};

/// `HRMS-706` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct PreventivePlanGateway {
    db: DbConn,
}

impl PreventivePlanGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<PreventivePlan, preventive_plan_entity::Model, preventive_plan_entity::ActiveModel>
    for PreventivePlanGateway
{
    async fn persist(
        &self,
        entity: PreventivePlan,
    ) -> Result<preventive_plan_entity::ActiveModel, DbErr> {
        PreventivePlanEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            PreventivePlanQuery::delete_many().filter(preventive_plan_entity::Column::Id.eq(id)),
            preventive_plan_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<preventive_plan_entity::Model>, DbErr> {
        tenant_select(PreventivePlanQuery::find(), preventive_plan_entity::Column::TenantId)
            .filter(preventive_plan_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<preventive_plan_entity::Model>, DbErr> {
        tenant_select(PreventivePlanQuery::find(), preventive_plan_entity::Column::TenantId)
            .filter(preventive_plan_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<preventive_plan_entity::Model>, DbErr> {
        tenant_select(PreventivePlanQuery::find(), preventive_plan_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl PreventivePlanGateway {
    /// `PD-028`: every preventive plan of the caller's tenant, by vehicle.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<preventive_plan_entity::Model>, u64), DbErr> {
        let query = tenant_select(PreventivePlanQuery::find(), preventive_plan_entity::Column::TenantId)
            .order_by_asc(preventive_plan_entity::Column::VehicleId);
        fetch_page(query, &self.db, page, page_size).await
    }

    /// `TRM-300`: the duplicate-refusal pre-check -- "at most one plan per
    /// vehicle and plan type."
    pub async fn find_by_vehicle_and_name(
        &self,
        vehicle_id: i64,
        plan_name: &str,
    ) -> Result<Option<preventive_plan_entity::Model>, DbErr> {
        tenant_select(PreventivePlanQuery::find(), preventive_plan_entity::Column::TenantId)
            .filter(
                Condition::all()
                    .add(preventive_plan_entity::Column::VehicleId.eq(vehicle_id))
                    .add(preventive_plan_entity::Column::PlanName.eq(plan_name)),
            )
            .one(&self.db)
            .await
    }
}

/// `HRMS-706` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::preventive_plan_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_preventive_plans() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                preventive_plan_entity::Entity::find(),
                preventive_plan_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_preventive_plan_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                preventive_plan_entity::Entity::find(),
                preventive_plan_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_preventive_plan_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                preventive_plan_entity::Entity::delete_many(),
                preventive_plan_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
