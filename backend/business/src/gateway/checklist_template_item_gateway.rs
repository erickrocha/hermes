use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::checklist_template_item::{ChecklistTemplateItem, ChecklistTemplateItemEntityMapper};
use entity::checklist_template_item_entity;
use entity::prelude::ChecklistTemplateItemEntity as ChecklistTemplateItemQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

/// `HRMS-651` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct ChecklistTemplateItemGateway {
    db: DbConn,
}

impl ChecklistTemplateItemGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<ChecklistTemplateItem, checklist_template_item_entity::Model, checklist_template_item_entity::ActiveModel>
    for ChecklistTemplateItemGateway
{
    async fn persist(
        &self,
        entity: ChecklistTemplateItem,
    ) -> Result<checklist_template_item_entity::ActiveModel, DbErr> {
        ChecklistTemplateItemEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            ChecklistTemplateItemQuery::delete_many()
                .filter(checklist_template_item_entity::Column::Id.eq(id)),
            checklist_template_item_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(
        &self,
        id: i64,
    ) -> Result<Option<checklist_template_item_entity::Model>, DbErr> {
        tenant_select(
            ChecklistTemplateItemQuery::find(),
            checklist_template_item_entity::Column::TenantId,
        )
        .filter(checklist_template_item_entity::Column::Id.eq(id))
        .one(&self.db)
        .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<checklist_template_item_entity::Model>, DbErr> {
        tenant_select(
            ChecklistTemplateItemQuery::find(),
            checklist_template_item_entity::Column::TenantId,
        )
        .filter(checklist_template_item_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
        .one(&self.db)
        .await
    }

    async fn find_all(&self) -> Result<Vec<checklist_template_item_entity::Model>, DbErr> {
        tenant_select(
            ChecklistTemplateItemQuery::find(),
            checklist_template_item_entity::Column::TenantId,
        )
        .all(&self.db)
        .await
    }
}

impl ChecklistTemplateItemGateway {
    /// A template's items, in the order they were added.
    pub async fn find_by_template(
        &self,
        checklist_template_id: i64,
    ) -> Result<Vec<checklist_template_item_entity::Model>, DbErr> {
        tenant_select(
            ChecklistTemplateItemQuery::find(),
            checklist_template_item_entity::Column::TenantId,
        )
        .filter(checklist_template_item_entity::Column::ChecklistTemplateId.eq(checklist_template_id))
        .order_by_asc(checklist_template_item_entity::Column::Id)
        .all(&self.db)
        .await
    }
}

/// `HRMS-651` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::checklist_template_item_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_items() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                checklist_template_item_entity::Entity::find(),
                checklist_template_item_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_item_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                checklist_template_item_entity::Entity::find(),
                checklist_template_item_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_an_item_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                checklist_template_item_entity::Entity::delete_many(),
                checklist_template_item_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
