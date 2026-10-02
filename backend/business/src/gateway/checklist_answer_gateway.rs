use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::checklist_answer::{ChecklistAnswer, ChecklistAnswerEntityMapper};
use entity::checklist_answer_entity;
use entity::prelude::ChecklistAnswerEntity as ChecklistAnswerQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

/// `HRMS-654` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct ChecklistAnswerGateway {
    db: DbConn,
}

impl ChecklistAnswerGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<ChecklistAnswer, checklist_answer_entity::Model, checklist_answer_entity::ActiveModel>
    for ChecklistAnswerGateway
{
    async fn persist(
        &self,
        entity: ChecklistAnswer,
    ) -> Result<checklist_answer_entity::ActiveModel, DbErr> {
        ChecklistAnswerEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            ChecklistAnswerQuery::delete_many().filter(checklist_answer_entity::Column::Id.eq(id)),
            checklist_answer_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(
        &self,
        id: i64,
    ) -> Result<Option<checklist_answer_entity::Model>, DbErr> {
        tenant_select(ChecklistAnswerQuery::find(), checklist_answer_entity::Column::TenantId)
            .filter(checklist_answer_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<checklist_answer_entity::Model>, DbErr> {
        tenant_select(ChecklistAnswerQuery::find(), checklist_answer_entity::Column::TenantId)
            .filter(checklist_answer_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<checklist_answer_entity::Model>, DbErr> {
        tenant_select(ChecklistAnswerQuery::find(), checklist_answer_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl ChecklistAnswerGateway {
    /// A run's answers, in the order they were recorded.
    pub async fn find_by_run(
        &self,
        checklist_run_id: i64,
    ) -> Result<Vec<checklist_answer_entity::Model>, DbErr> {
        tenant_select(ChecklistAnswerQuery::find(), checklist_answer_entity::Column::TenantId)
            .filter(checklist_answer_entity::Column::ChecklistRunId.eq(checklist_run_id))
            .order_by_asc(checklist_answer_entity::Column::Id)
            .all(&self.db)
            .await
    }
}

/// `HRMS-654` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::checklist_answer_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_answers() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                checklist_answer_entity::Entity::find(),
                checklist_answer_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_answer_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                checklist_answer_entity::Entity::find(),
                checklist_answer_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_an_answer_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                checklist_answer_entity::Entity::delete_many(),
                checklist_answer_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
