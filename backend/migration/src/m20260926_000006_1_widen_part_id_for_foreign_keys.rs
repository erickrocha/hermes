use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_table("part").await? {
            return Ok(());
        }
        manager
            .alter_table(
                Table::alter()
                    .table(Part::Table)
                    .modify_column(ColumnDef::new(Part::Id).big_integer().not_null().auto_increment())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_table("part").await? {
            return Ok(());
        }
        manager
            .alter_table(
                Table::alter()
                    .table(Part::Table)
                    .modify_column(ColumnDef::new(Part::Id).integer().not_null().auto_increment())
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Part {
    Table,
    Id,
}