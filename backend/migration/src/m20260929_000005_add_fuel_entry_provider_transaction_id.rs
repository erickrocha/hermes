use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-FU-02-S01` (`HRMS-944`, `TRM-511`): the fuel-management provider's
/// own transaction identifier, so a re-sent transaction is recognised as a
/// correction (`TRM-512`), not re-imported as a duplicate. `EPIC-FU-01-S01`
/// deliberately left this off the manual-entry-only ledger; a manual entry
/// has no provider transaction to reference. No column for "the provider's
/// own vehicle reference" (`TRM-514`'s other field) -- it is only ever used
/// to *match* a vehicle at import time (`TRM-507`), never read again once
/// `vehicle_id`, a real FK, is set.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(FuelEntry::Table)
                    .add_column(string_len_null(FuelEntry::ProviderTransactionId, 100))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_fuel_entry_provider_transaction")
                    .table(FuelEntry::Table)
                    .col(FuelEntry::ProviderTransactionId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(FuelEntry::Table)
                    .drop_column(FuelEntry::ProviderTransactionId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum FuelEntry {
    Table,
    ProviderTransactionId,
}
