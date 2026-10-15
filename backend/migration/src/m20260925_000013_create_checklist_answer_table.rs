use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-CK-04-S01` (`HRMS-654`, `C-025`): a driver's answer to one
/// template item of the checklist they submitted. Unique on
/// `(checklist_run_id, checklist_template_item_id)` -- an item may be
/// answered at most once per run, enforced at the DB level too.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ChecklistAnswer::Table)
                    .if_not_exists()
                    .col(pk_auto(ChecklistAnswer::Id).integer())
                    .col(binary_len_uniq(ChecklistAnswer::Uuid, 16))
                    .col(integer(ChecklistAnswer::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_checklist_answer_tenant")
                            .from(ChecklistAnswer::Table, ChecklistAnswer::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(ChecklistAnswer::ChecklistRunId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_checklist_answer_run")
                            .from(ChecklistAnswer::Table, ChecklistAnswer::ChecklistRunId)
                            .to(ChecklistRun::Table, ChecklistRun::Id),
                    )
                    .col(integer(ChecklistAnswer::ChecklistTemplateItemId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_checklist_answer_template_item")
                            .from(ChecklistAnswer::Table, ChecklistAnswer::ChecklistTemplateItemId)
                            .to(ChecklistTemplateItem::Table, ChecklistTemplateItem::Id),
                    )
                    .col(string_len(ChecklistAnswer::Status, 50).not_null())
                    .col(string_len_null(ChecklistAnswer::Observation, 500))
                    .col(date_time(ChecklistAnswer::CreatedAt).null())
                    .col(string_len(ChecklistAnswer::CreatedBy, 50).null())
                    .col(date_time(ChecklistAnswer::UpdatedAt).null())
                    .col(string_len(ChecklistAnswer::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_checklist_answer_run_item")
                    .table(ChecklistAnswer::Table)
                    .col(ChecklistAnswer::ChecklistRunId)
                    .col(ChecklistAnswer::ChecklistTemplateItemId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ChecklistAnswer::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum ChecklistAnswer {
    Table,
    Id,
    Uuid,
    TenantId,
    ChecklistRunId,
    ChecklistTemplateItemId,
    Status,
    Observation,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum Tenant {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ChecklistRun {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ChecklistTemplateItem {
    Table,
    Id,
}
