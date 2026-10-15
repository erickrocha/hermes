use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-FU-04-S01` (`HRMS-949`, `TRM-548`): when the provider confirmed a
/// driver-reported fuelling it was reconciled with. Null until then.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(FuelEntry::Table)
                    .add_column(date_time_null(FuelEntry::ProviderConfirmedAt))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(Table::alter().table(FuelEntry::Table).drop_column(FuelEntry::ProviderConfirmedAt).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum FuelEntry {
    Table,
    ProviderConfirmedAt,
}
