use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::business_error::BusinessError;
use crate::domain::checklist_template::{ChecklistTemplate, ChecklistTemplateEntityMapper};
use crate::domain::checklist_template_item::{
    ChecklistTemplateItem, ChecklistTemplateItemEntityMapper,
};
use crate::gateway::checklist_template_item_gateway::ChecklistTemplateItemGateway;
use entity::checklist_template_entity;
use entity::prelude::ChecklistTemplateEntity as ChecklistTemplateQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};

#[async_trait]
pub trait ChecklistTemplateStore: Send + Sync {
    async fn create(
        &self,
        template: ChecklistTemplate,
        items: Vec<ChecklistTemplateItem>,
    ) -> Result<(ChecklistTemplate, Vec<ChecklistTemplateItem>), BusinessError>;
    async fn get_by_uuid(&self, uuid: String) -> Result<Option<ChecklistTemplate>, BusinessError>;
    async fn list_items(
        &self,
        template_id: i64,
    ) -> Result<Vec<ChecklistTemplateItem>, BusinessError>;
    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<ChecklistTemplate>, u64), BusinessError>;
    /// `SR-CA-03-09`: makes `template` the default of its tenant and type and
    /// clears the previous one, in one transaction.
    async fn set_default(
        &self,
        template: ChecklistTemplate,
    ) -> Result<ChecklistTemplate, BusinessError>;
}

/// `HRMS-651` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct ChecklistTemplateGateway {
    db: DbConn,
}

impl ChecklistTemplateGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    /// `PD-027`-style all-or-nothing create: the use case inserts the
    /// template and its items in one transaction started from here.
    pub fn db(&self) -> &DbConn {
        &self.db
    }
}

pub struct ChecklistTemplateDatabaseGateway {
    templates: ChecklistTemplateGateway,
    items: ChecklistTemplateItemGateway,
}

impl ChecklistTemplateDatabaseGateway {
    pub fn new(templates: ChecklistTemplateGateway, items: ChecklistTemplateItemGateway) -> Self {
        Self { templates, items }
    }
}

#[async_trait]
impl ChecklistTemplateStore for ChecklistTemplateDatabaseGateway {
    async fn create(
        &self,
        template: ChecklistTemplate,
        items: Vec<ChecklistTemplateItem>,
    ) -> Result<(ChecklistTemplate, Vec<ChecklistTemplateItem>), BusinessError> {
        let transaction = self.templates.db().begin().await.map_err(database_error)?;
        let active = match ChecklistTemplateEntityMapper::build_active_model(template)
            .save(&transaction)
            .await
        {
            Ok(active) => active,
            Err(error) => {
                let _ = transaction.rollback().await;
                return Err(database_error(error));
            }
        };
        let template = ChecklistTemplateEntityMapper::from_active_model(active);
        let mut saved_items = Vec::with_capacity(items.len());
        for mut item in items {
            item.checklist_template_id = template.id.unwrap_or_default();
            // An item belongs to its template's tenant. A tenant-bound caller gets this stamped on save;
            // the platform administrator has no tenant of their own, so it must come from the template.
            item.tenant_id = template.tenant_id;
            match ChecklistTemplateItemEntityMapper::build_active_model(item)
                .save(&transaction)
                .await
            {
                Ok(active) => {
                    saved_items.push(ChecklistTemplateItemEntityMapper::from_active_model(active))
                }
                Err(error) => {
                    let _ = transaction.rollback().await;
                    return Err(database_error(error));
                }
            }
        }
        transaction.commit().await.map_err(database_error)?;
        Ok((template, saved_items))
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<ChecklistTemplate>, BusinessError> {
        <ChecklistTemplateGateway as Gateway<
            ChecklistTemplate,
            checklist_template_entity::Model,
            checklist_template_entity::ActiveModel,
        >>::find_by_uuid(&self.templates, uuid)
        .await
        .map(|model| model.map(ChecklistTemplateEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn list_items(
        &self,
        template_id: i64,
    ) -> Result<Vec<ChecklistTemplateItem>, BusinessError> {
        self.items
            .find_by_template(template_id)
            .await
            .map(ChecklistTemplateItemEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<ChecklistTemplate>, u64), BusinessError> {
        self.templates
            .find_page(page, page_size)
            .await
            .map(|(models, total)| (ChecklistTemplateEntityMapper::from_models(models), total))
            .map_err(database_error)
    }

    async fn set_default(
        &self,
        template: ChecklistTemplate,
    ) -> Result<ChecklistTemplate, BusinessError> {
        let id = template.id.unwrap_or_default();
        let transaction = self.templates.db().begin().await.map_err(database_error)?;
        match Self::swap_default(&transaction, &template, id).await {
            Ok(updated) => {
                transaction.commit().await.map_err(database_error)?;
                Ok(ChecklistTemplateEntityMapper::from_model(updated))
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(database_error(error))
            }
        }
    }
}

impl ChecklistTemplateDatabaseGateway {
    /// Each row goes through `ActiveModel::update` (not `update_many`) so the
    /// audit columns are stamped like every other write.
    async fn swap_default(
        transaction: &sea_orm::DatabaseTransaction,
        template: &ChecklistTemplate,
        id: i64,
    ) -> Result<checklist_template_entity::Model, DbErr> {
        use checklist_template_entity::Column;
        let same_tenant = match template.tenant_id {
            Some(tenant_id) => Column::TenantId.eq(tenant_id),
            None => Column::TenantId.is_null(),
        };
        let current = ChecklistTemplateQuery::find()
            .filter(same_tenant)
            .filter(Column::ChecklistType.eq(template.checklist_type.to_string()))
            .filter(Column::IsDefault.eq(true))
            .filter(Column::Id.ne(id))
            .all(transaction)
            .await?;
        for model in current {
            let mut active: checklist_template_entity::ActiveModel = model.into();
            active.is_default = Set(false);
            active.update(transaction).await?;
        }
        let target = ChecklistTemplateQuery::find_by_id(id)
            .one(transaction)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound(format!("checklist_template {id}")))?;
        let mut active: checklist_template_entity::ActiveModel = target.into();
        active.is_default = Set(true);
        active.update(transaction).await
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Database error: {error}");
    log::error!("[ChecklistTemplateGateway] {message}");
    BusinessError::new(message)
}

#[async_trait]
impl
    Gateway<
        ChecklistTemplate,
        checklist_template_entity::Model,
        checklist_template_entity::ActiveModel,
    > for ChecklistTemplateGateway
{
    async fn persist(
        &self,
        entity: ChecklistTemplate,
    ) -> Result<checklist_template_entity::ActiveModel, DbErr> {
        ChecklistTemplateEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            ChecklistTemplateQuery::delete_many()
                .filter(checklist_template_entity::Column::Id.eq(id)),
            checklist_template_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<checklist_template_entity::Model>, DbErr> {
        tenant_select(
            ChecklistTemplateQuery::find(),
            checklist_template_entity::Column::TenantId,
        )
        .filter(checklist_template_entity::Column::Id.eq(id))
        .one(&self.db)
        .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<checklist_template_entity::Model>, DbErr> {
        tenant_select(
            ChecklistTemplateQuery::find(),
            checklist_template_entity::Column::TenantId,
        )
        .filter(checklist_template_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
        .one(&self.db)
        .await
    }

    async fn find_all(&self) -> Result<Vec<checklist_template_entity::Model>, DbErr> {
        tenant_select(
            ChecklistTemplateQuery::find(),
            checklist_template_entity::Column::TenantId,
        )
        .all(&self.db)
        .await
    }
}

impl ChecklistTemplateGateway {
    /// `PD-028`: every template of the caller's tenant, most recently
    /// created first.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<checklist_template_entity::Model>, u64), DbErr> {
        use sea_orm::QueryOrder;
        let query = tenant_select(
            ChecklistTemplateQuery::find(),
            checklist_template_entity::Column::TenantId,
        )
        .order_by_desc(checklist_template_entity::Column::Id);
        fetch_page(query, &self.db, page, page_size).await
    }
}

/// `HRMS-651` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::checklist_template_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_templates() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                checklist_template_entity::Entity::find(),
                checklist_template_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_template_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                checklist_template_entity::Entity::find(),
                checklist_template_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_template_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                checklist_template_entity::Entity::delete_many(),
                checklist_template_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
