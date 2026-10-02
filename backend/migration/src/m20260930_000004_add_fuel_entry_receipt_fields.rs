use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-FU-03-S01` (`HRMS-948`, `TRM-535`/`TRM-540`): who reported a
/// driver-confirmed receipt, and -- when the driver overrode a divergent
/// odometer -- the audit note holding the reference value and both window
/// bounds.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(FuelEntry::Table)
                    .add_column(integer_null(FuelEntry::ReportedByUserId))
                    .add_column(string_len_null(FuelEntry::OdometerOverrideNote, 300))
                    .add_foreign_key(
                        TableForeignKey::new()
                            .name("fk_fuel_entry_reported_by")
                            .from_tbl(FuelEntry::Table)
                            .from_col(FuelEntry::ReportedByUserId)
                            .to_tbl(User::Table)
                            .to_col(User::Id),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(FuelEntry::Table)
                    .drop_foreign_key(Alias::new("fk_fuel_entry_reported_by"))
                    .drop_column(FuelEntry::ReportedByUserId)
                    .drop_column(FuelEntry::OdometerOverrideNote)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum FuelEntry {
    Table,
    ReportedByUserId,
    OdometerOverrideNote,
}

#[derive(DeriveIden)]
enum User {
    Table,
    Id,
}
