use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{fetch_page, tenant_delete, tenant_select, Gateway};
use crate::domain::vehicle::{Vehicle, VehicleEntityMapper};
use entity::prelude::VehicleEntity as VehicleQuery;
use entity::vehicle_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

/// EPIC-FO-01-S02 (HRMS-921, D-09): every read and delete here goes through
/// `tenant_select`/`tenant_delete`. The write side is stamped by the entity's
/// `impl_tenant_auditable_before_save!`; on its own that buys nothing on the
/// read side, which is the half defect D-7 was missing. `UserGateway` is the
/// reference implementation this file copies, and
/// `business/tests/tenant_scoping_rule.rs` is what refuses a vehicle gateway
/// that skips either call.
pub struct VehicleGateway {
    db: DbConn,
}

impl VehicleGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &DbConn {
        &self.db
    }
}

#[async_trait]
impl Gateway<Vehicle, vehicle_entity::Model, vehicle_entity::ActiveModel> for VehicleGateway {
    async fn persist(&self, entity: Vehicle) -> Result<vehicle_entity::ActiveModel, DbErr> {
        let active_model = VehicleEntityMapper::build_active_model(entity);
        active_model.save(&self.db).await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            VehicleQuery::delete_many().filter(vehicle_entity::Column::Id.eq(id)),
            vehicle_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<vehicle_entity::Model>, DbErr> {
        tenant_select(VehicleQuery::find(), vehicle_entity::Column::TenantId)
            .filter(vehicle_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<vehicle_entity::Model>, DbErr> {
        tenant_select(VehicleQuery::find(), vehicle_entity::Column::TenantId)
            .filter(vehicle_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<vehicle_entity::Model>, DbErr> {
        tenant_select(VehicleQuery::find(), vehicle_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl VehicleGateway {
    /// PD-028. Still goes through `tenant_select`: paging must not be a read
    /// path that escapes the tenant scope.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<vehicle_entity::Model>, u64), DbErr> {
        let mut query = tenant_select(VehicleQuery::find(), vehicle_entity::Column::TenantId)
            .order_by_desc(vehicle_entity::Column::Id);
        if let Some(term) = search {
            query = query.filter(
                Condition::any()
                    .add(vehicle_entity::Column::Plate.contains(term))
                    .add(vehicle_entity::Column::Model.contains(term)),
            );
        }
        fetch_page(query, &self.db, page, page_size).await
    }

    /// EPIC-FO-01-S06 (HRMS-925, D-23(c)): the duplicate check that makes
    /// `uq_vehicle_tenant_plate` a reportable error rather than a raw
    /// constraint violation. Scoped, like every other read here -- a global
    /// lookup would answer "taken" for a plate registered by another customer,
    /// which is exactly the disclosure D-23(c) rejected platform-wide
    /// uniqueness to avoid.
    pub async fn find_by_plate(&self, plate: &str) -> Result<Option<vehicle_entity::Model>, DbErr> {
        tenant_select(VehicleQuery::find(), vehicle_entity::Column::TenantId)
            .filter(vehicle_entity::Column::Plate.eq(plate))
            .one(&self.db)
            .await
    }
}

/// HRMS-921 (EPIC-FO-01-S02): the read side of the scoping rule, proved on the
/// query builder rather than against a database -- the same technique the
/// `user` tests in `commons/gateway.rs` use, and for the same reason: the
/// property is about the SQL that gets built.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{run_with_user, AuditUser};
    use entity::vehicle_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_vehicles() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(vehicle_entity::Entity::find(), vehicle_entity::Column::TenantId)
                .build(DbBackend::MySql)
                .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_vehicle_at_all() {
        // D-05: outside a request the scope is Denied, not unrestricted.
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(vehicle_entity::Entity::find(), vehicle_entity::Column::TenantId)
                .build(DbBackend::MySql)
                .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn an_unrestricted_caller_adds_no_filter_to_a_vehicle_read() {
        let unscoped = vehicle_entity::Entity::find().build(DbBackend::MySql).to_string();
        let scoped = run_with_user(Some(caller(None, false)), async {
            tenant_select(vehicle_entity::Entity::find(), vehicle_entity::Column::TenantId)
                .build(DbBackend::MySql)
                .to_string()
        })
        .await;
        assert_eq!(scoped, unscoped);
    }

    #[tokio::test]
    async fn deleting_a_vehicle_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                vehicle_entity::Entity::delete_many(),
                vehicle_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");

        let denied = run_with_user(Some(caller(None, true)), async {
            tenant_delete(
                vehicle_entity::Entity::delete_many(),
                vehicle_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(denied.contains("1 = 0"), "{denied}");
    }
}
