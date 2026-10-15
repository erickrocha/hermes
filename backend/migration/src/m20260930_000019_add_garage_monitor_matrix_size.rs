use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-GA-07-S04` (`HRMS-967`, `C-028`): the services matrix's size, `[TC]` 10 rows x 8 columns (`TRM-496`).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Setting::Table)
                    .add_column(integer(Setting::GarageMonitorMatrixRows).not_null().default(10))
                    .add_column(integer(Setting::GarageMonitorMatrixColumns).not_null().default(8))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Setting::Table)
                    .drop_column(Setting::GarageMonitorMatrixRows)
                    .drop_column(Setting::GarageMonitorMatrixColumns)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Setting {
    #[sea_orm(iden = "tenant_rule_setting")]
    Table,
    GarageMonitorMatrixRows,
    GarageMonitorMatrixColumns,
}
