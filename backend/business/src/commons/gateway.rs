use sea_orm::{ColumnTrait, DbErr, DeleteMany, DeleteResult, EntityTrait, QueryFilter, Select};
use sea_orm::sea_query::Expr;
use sea_orm::prelude::async_trait::async_trait;
use entity::audit;

// EPIC-XF-01-S10 convention (HRMS-010, AD-015): tenancy is opt-in per entity,
// declared by which `entity::audit` macro an entity's `ActiveModel` invokes —
// `impl_tenant_auditable_before_save!` (tenant-stamped) or
// `impl_auditable_before_save!` (audited only, e.g. the global business-plan
// catalogue). That macro choice is only *half* the declaration: a gateway
// method that reads or deletes rows must also call `tenant_select`/
// `tenant_delete` below, or the write-side stamping buys nothing on the read
// side. There is deliberately no compiler or test enforcement pairing the
// two yet (AD-015's own stated risk) — `UserGateway` is the reference
// implementation to copy when a new tenant-owned table is added. Today
// `user` is the only entity using the tenant macro; see D-7 in
// `01-project_truth/hermes/04-unknowns/open-questions.md`.

#[async_trait]
pub trait Gateway<D, M, AM> {
	async fn persist(&self, domain: D) -> Result<AM, DbErr>;
	async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr>;
	async fn find_by_id(&self, id: i64) -> Result<Option<M>, DbErr>;
	async fn find_by_uuid(&self, uuid: String) -> Result<Option<M>, DbErr>;
	async fn find_all(&self) -> Result<Vec<M>, DbErr>;
}

pub fn tenant_select<E, C>(query: Select<E>, tenant_column: C) -> Select<E>
where
    E: EntityTrait,
    C: ColumnTrait,
{
    match audit::tenant_scope() {
        audit::TenantScope::Unrestricted => query,
        audit::TenantScope::Tenant(id) => query.filter(tenant_column.eq(id)),
        audit::TenantScope::Denied => query.filter(Expr::cust("1 = 0")),
    }
}

pub fn tenant_delete<E, C>(query: DeleteMany<E>, tenant_column: C) -> DeleteMany<E>
where
    E: EntityTrait,
    C: ColumnTrait,
{
    match audit::tenant_scope() {
        audit::TenantScope::Unrestricted => query,
        audit::TenantScope::Tenant(id) => query.filter(tenant_column.eq(id)),
        audit::TenantScope::Denied => query.filter(Expr::cust("1 = 0")),
    }
}

/// HRMS-011 (EPIC-XF-01-S11): proves `tenant_select`/`tenant_delete` build the
/// SQL their three scopes promise, against the one entity (`user`) currently
/// wired for read-side scoping. Uses the real query builder against a
/// `DbBackend`, not a live database — no `mock` feature needed for this half
/// of the mechanism.
#[cfg(test)]
mod tests {
    use super::{tenant_delete, tenant_select};
    use entity::audit::{run_with_user, AuditUser};
    use entity::user_entity;
    use sea_orm::{DbBackend, EntityTrait, QueryTrait};

    fn user(tenant_id: Option<i64>, enforce_tenant: bool) -> AuditUser {
        AuditUser {
            id: 1,
            email: "user@example.com".to_string(),
            tenant_id,
            enforce_tenant,
        }
    }

    #[tokio::test]
    async fn unrestricted_scope_adds_no_tenant_filter() {
        // `tenant_id` legitimately appears in the column list of any `SELECT *`
        // on `user` — the thing to prove is that unrestricted scope adds no
        // *filter*, i.e. the scoped query is identical to the unscoped one.
        let unscoped_sql = user_entity::Entity::find().build(DbBackend::MySql).to_string();
        let scoped_sql = run_with_user(Some(user(None, false)), async {
            tenant_select(user_entity::Entity::find(), user_entity::Column::TenantId)
                .build(DbBackend::MySql)
                .to_string()
        })
        .await;
        assert_eq!(scoped_sql, unscoped_sql);
    }

    #[tokio::test]
    async fn tenant_scope_filters_reads_by_tenant_id() {
        let sql = run_with_user(Some(user(Some(42), true)), async {
            tenant_select(user_entity::Entity::find(), user_entity::Column::TenantId)
                .build(DbBackend::MySql)
                .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"));
        assert!(sql.contains("42"));
    }

    #[tokio::test]
    async fn denied_scope_forces_reads_to_return_no_rows() {
        let sql = run_with_user(Some(user(None, true)), async {
            tenant_select(user_entity::Entity::find(), user_entity::Column::TenantId)
                .build(DbBackend::MySql)
                .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"));
    }

    #[tokio::test]
    async fn tenant_delete_is_scoped_the_same_way_as_tenant_select() {
        let sql = run_with_user(Some(user(Some(7), true)), async {
            tenant_delete(user_entity::Entity::delete_many(), user_entity::Column::TenantId)
                .build(DbBackend::MySql)
                .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"));
        assert!(sql.contains('7'));
    }

    #[tokio::test]
    async fn denied_scope_forces_deletes_to_match_no_rows() {
        let sql = run_with_user(Some(user(None, true)), async {
            tenant_delete(user_entity::Entity::delete_many(), user_entity::Column::TenantId)
                .build(DbBackend::MySql)
                .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"));
    }
}

