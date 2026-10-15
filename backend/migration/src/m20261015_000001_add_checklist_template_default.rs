use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `C-031` (`SR-CA-03-09`): the tenant's default template per checklist type.
/// At most one default per tenant and type is kept by the use case, not by a
/// constraint (MariaDB has no partial unique index).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ChecklistTemplate::Table)
                    .add_column(
                        ColumnDef::new(ChecklistTemplate::IsDefault)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ChecklistTemplate::Table)
                    .drop_column(ChecklistTemplate::IsDefault)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ChecklistTemplate {
    Table,
    IsDefault,
}
