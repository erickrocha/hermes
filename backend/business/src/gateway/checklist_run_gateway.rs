use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::checklist_run::{ChecklistRun, ChecklistRunEntityMapper};
use entity::checklist_run_entity;
use entity::prelude::ChecklistRunEntity as ChecklistRunQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

/// `HRMS-652` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct ChecklistRunGateway {
    db: DbConn,
}

impl ChecklistRunGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    /// `EPIC-CK-04-S01`: the use case writes the run and its answers in one
    /// transaction started from here -- the same `PD-027`-style
    /// all-or-nothing technique `ChecklistTemplateGateway::db` already uses.
    pub fn db(&self) -> &DbConn {
        &self.db
    }
}

#[async_trait]
impl Gateway<ChecklistRun, checklist_run_entity::Model, checklist_run_entity::ActiveModel>
    for ChecklistRunGateway
{
    async fn persist(
        &self,
        entity: ChecklistRun,
    ) -> Result<checklist_run_entity::ActiveModel, DbErr> {
        ChecklistRunEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            ChecklistRunQuery::delete_many().filter(checklist_run_entity::Column::Id.eq(id)),
            checklist_run_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<checklist_run_entity::Model>, DbErr> {
        tenant_select(ChecklistRunQuery::find(), checklist_run_entity::Column::TenantId)
            .filter(checklist_run_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<checklist_run_entity::Model>, DbErr> {
        tenant_select(ChecklistRunQuery::find(), checklist_run_entity::Column::TenantId)
            .filter(checklist_run_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<checklist_run_entity::Model>, DbErr> {
        tenant_select(ChecklistRunQuery::find(), checklist_run_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl ChecklistRunGateway {
    /// `TRM-105`: every `Departure` run this driver has ever opened, most
    /// recent first. The use case decides which of these, if any, is still
    /// open (has no `Return` naming it) -- that needs the full list, not
    /// just the latest one, since only a `Return` row proves a departure is
    /// closed.
    pub async fn find_departures_by_driver(
        &self,
        driver_id: i64,
    ) -> Result<Vec<checklist_run_entity::Model>, DbErr> {
        tenant_select(ChecklistRunQuery::find(), checklist_run_entity::Column::TenantId)
            .filter(checklist_run_entity::Column::DriverId.eq(driver_id))
            .filter(checklist_run_entity::Column::ChecklistType.eq("Departure"))
            .order_by_desc(checklist_run_entity::Column::CreatedAt)
            .all(&self.db)
            .await
    }

    /// The `Return` rows (if any) that close the given `Departure` ids --
    /// at most one per id (`uq_checklist_run_opening_checklist`).
    pub async fn find_returns_closing(
        &self,
        opening_checklist_ids: Vec<i64>,
    ) -> Result<Vec<checklist_run_entity::Model>, DbErr> {
        tenant_select(ChecklistRunQuery::find(), checklist_run_entity::Column::TenantId)
            .filter(checklist_run_entity::Column::OpeningChecklistId.is_in(opening_checklist_ids))
            .all(&self.db)
            .await
    }

    /// `TRM-123`-lite: the vehicle's most recent `Departure` run, whether or
    /// not it has since been closed -- the use case checks the latter via
    /// `find_returns_closing`.
    pub async fn find_latest_departure_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<checklist_run_entity::Model>, DbErr> {
        tenant_select(ChecklistRunQuery::find(), checklist_run_entity::Column::TenantId)
            .filter(checklist_run_entity::Column::VehicleId.eq(vehicle_id))
            .filter(checklist_run_entity::Column::ChecklistType.eq("Departure"))
            .order_by_desc(checklist_run_entity::Column::CreatedAt)
            .one(&self.db)
            .await
    }
}

/// `HRMS-652` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::checklist_run_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_runs() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                checklist_run_entity::Entity::find(),
                checklist_run_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_run_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                checklist_run_entity::Entity::find(),
                checklist_run_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_run_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                checklist_run_entity::Entity::delete_many(),
                checklist_run_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
