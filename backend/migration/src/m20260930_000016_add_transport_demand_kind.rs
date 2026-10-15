use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-SC-04-S01` (`HRMS-610`, `TRM-002`): the demand's stated kind --
/// `Line`, `ExtraLine` or `OneOffTrip`. Nullable: existing demands keep their
/// free-text `demand_type` untouched and are simply unclassified until the owner
/// classifies them; nothing is inferred from the text.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter().table(Demand::Table).add_column(string_len_null(Demand::DemandKind, 20)).to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.alter_table(Table::alter().table(Demand::Table).drop_column(Demand::DemandKind).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Demand {
    #[sea_orm(iden = "transport_demand")]
    Table,
    DemandKind,
}
