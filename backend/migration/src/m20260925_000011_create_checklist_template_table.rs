use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-CK-02-S01` (`HRMS-651`, `C-025`): a reusable driver-checklist
/// template plus its items, one row each so a future checklist answer
/// (`EPIC-CK-04`) can reference the exact item it answers.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ChecklistTemplate::Table)
                    .if_not_exists()
                    .col(pk_auto(ChecklistTemplate::Id).integer())
                    .col(binary_len_uniq(ChecklistTemplate::Uuid, 16))
                    .col(integer(ChecklistTemplate::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_checklist_template_tenant")
                            .from(ChecklistTemplate::Table, ChecklistTemplate::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(string_len(ChecklistTemplate::Name, 255).not_null())
                    .col(string_len(ChecklistTemplate::ChecklistType, 50).not_null())
                    .col(boolean(ChecklistTemplate::Active).not_null())
                    .col(date_time(ChecklistTemplate::CreatedAt).null())
                    .col(string_len(ChecklistTemplate::CreatedBy, 50).null())
                    .col(date_time(ChecklistTemplate::UpdatedAt).null())
                    .col(string_len(ChecklistTemplate::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ChecklistTemplateItem::Table)
                    .if_not_exists()
                    .col(pk_auto(ChecklistTemplateItem::Id).integer())
                    .col(binary_len_uniq(ChecklistTemplateItem::Uuid, 16))
                    .col(integer(ChecklistTemplateItem::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_checklist_template_item_tenant")
                            .from(ChecklistTemplateItem::Table, ChecklistTemplateItem::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(ChecklistTemplateItem::ChecklistTemplateId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_checklist_template_item_template")
                            .from(ChecklistTemplateItem::Table, ChecklistTemplateItem::ChecklistTemplateId)
                            .to(ChecklistTemplate::Table, ChecklistTemplate::Id),
                    )
                    .col(string_len(ChecklistTemplateItem::Description, 255).not_null())
                    .col(boolean(ChecklistTemplateItem::GeneratesWorkOrder).not_null())
                    .col(date_time(ChecklistTemplateItem::CreatedAt).null())
                    .col(string_len(ChecklistTemplateItem::CreatedBy, 50).null())
                    .col(date_time(ChecklistTemplateItem::UpdatedAt).null())
                    .col(string_len(ChecklistTemplateItem::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_checklist_template_item_template")
                    .table(ChecklistTemplateItem::Table)
                    .col(ChecklistTemplateItem::ChecklistTemplateId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ChecklistTemplateItem::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ChecklistTemplate::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum ChecklistTemplate {
    Table,
    Id,
    Uuid,
    TenantId,
    Name,
    ChecklistType,
    Active,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
pub enum ChecklistTemplateItem {
    Table,
    Id,
    Uuid,
    TenantId,
    ChecklistTemplateId,
    Description,
    GeneratesWorkOrder,
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
