use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::vehicle_expense::{VehicleExpense, VehicleExpenseEntityMapper};
use entity::prelude::VehicleExpenseEntity as VehicleExpenseQuery;
use entity::vehicle_expense_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DbConn, DbErr, DeleteResult, EntityTrait,
    QueryFilter, QueryOrder,
};

/// `HRMS-803` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct VehicleExpenseGateway {
    db: DbConn,
}

impl VehicleExpenseGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<VehicleExpense, vehicle_expense_entity::Model, vehicle_expense_entity::ActiveModel>
    for VehicleExpenseGateway
{
    async fn persist(
        &self,
        entity: VehicleExpense,
    ) -> Result<vehicle_expense_entity::ActiveModel, DbErr> {
        VehicleExpenseEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            VehicleExpenseQuery::delete_many().filter(vehicle_expense_entity::Column::Id.eq(id)),
            vehicle_expense_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<vehicle_expense_entity::Model>, DbErr> {
        tenant_select(
            VehicleExpenseQuery::find(),
            vehicle_expense_entity::Column::TenantId,
        )
        .filter(vehicle_expense_entity::Column::Id.eq(id))
        .one(&self.db)
        .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<vehicle_expense_entity::Model>, DbErr> {
        tenant_select(
            VehicleExpenseQuery::find(),
            vehicle_expense_entity::Column::TenantId,
        )
        .filter(vehicle_expense_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
        .one(&self.db)
        .await
    }

    async fn find_all(&self) -> Result<Vec<vehicle_expense_entity::Model>, DbErr> {
        tenant_select(
            VehicleExpenseQuery::find(),
            vehicle_expense_entity::Column::TenantId,
        )
        .all(&self.db)
        .await
    }
}

impl VehicleExpenseGateway {
    /// `PD-028`: every vehicle expense of the caller's tenant, most recent
    /// first.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<vehicle_expense_entity::Model>, u64), DbErr> {
        let query = tenant_select(
            VehicleExpenseQuery::find(),
            vehicle_expense_entity::Column::TenantId,
        )
        .order_by_desc(vehicle_expense_entity::Column::IssueDate);
        fetch_page(query, &self.db, page, page_size).await
    }

    /// `TRM-661`: the pre-check behind the friendly duplicate-invoice
    /// refusal -- the DB's own `uq_vehicle_expense_invoice` index is the
    /// backstop, this is what turns it into `DUPLICATE_EXPENSE_INVOICE`
    /// instead of a raw constraint-violation `DbErr`.
    pub async fn find_by_vehicle_category_invoice(
        &self,
        vehicle_id: i64,
        category: &str,
        invoice_number: &str,
    ) -> Result<Vec<vehicle_expense_entity::Model>, DbErr> {
        tenant_select(
            VehicleExpenseQuery::find(),
            vehicle_expense_entity::Column::TenantId,
        )
        .filter(
            Condition::all()
                .add(vehicle_expense_entity::Column::VehicleId.eq(vehicle_id))
                .add(vehicle_expense_entity::Column::Category.eq(category))
                .add(vehicle_expense_entity::Column::InvoiceNumber.eq(invoice_number)),
        )
        .all(&self.db)
        .await
    }
}

/// `HRMS-803` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::vehicle_expense_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_vehicle_expenses() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                vehicle_expense_entity::Entity::find(),
                vehicle_expense_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_vehicle_expense_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                vehicle_expense_entity::Entity::find(),
                vehicle_expense_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_vehicle_expense_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                vehicle_expense_entity::Entity::delete_many(),
                vehicle_expense_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
