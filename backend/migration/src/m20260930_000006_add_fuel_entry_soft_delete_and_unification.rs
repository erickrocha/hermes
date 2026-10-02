use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-FU-04-S02` (`HRMS-950`, `TRM-552…554`): a fuelling is only ever
/// soft-deleted; an absorbed one names the entry it was unified into, and the
/// keeper carries an annotation of what it absorbed.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(FuelEntry::Table)
                    .add_column(date_time_null(FuelEntry::DeletedAt))
                    .add_column(integer_null(FuelEntry::UnifiedIntoId))
                    .add_column(string_len_null(FuelEntry::UnificationNote, 500))
                    .add_foreign_key(
                        TableForeignKey::new()
                            .name("fk_fuel_entry_unified_into")
                            .from_tbl(FuelEntry::Table)
                            .from_col(FuelEntry::UnifiedIntoId)
                            .to_tbl(FuelEntry::Table)
                            .to_col(FuelEntry::Id),
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
                    .drop_foreign_key(Alias::new("fk_fuel_entry_unified_into"))
                    .drop_column(FuelEntry::DeletedAt)
                    .drop_column(FuelEntry::UnifiedIntoId)
                    .drop_column(FuelEntry::UnificationNote)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum FuelEntry {
    Table,
    Id,
    DeletedAt,
    UnifiedIntoId,
    UnificationNote,
}
