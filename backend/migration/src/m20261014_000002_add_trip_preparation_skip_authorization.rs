use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ExtraTrip::Table)
                    .add_column(string_len_null(ExtraTrip::PreparationSkipAuthorizedBy, 255))
                    .add_column(date_time_null(ExtraTrip::PreparationSkipAuthorizedAt))
                    .add_column(string_len_null(ExtraTrip::PreparationSkipJustification, 1000))
                    .add_column(integer_null(ExtraTrip::PreparationSkipVehicleId))
                    .add_column(date_null(ExtraTrip::PreparationSkipTripDate))
                    .add_column(time_null(ExtraTrip::PreparationSkipStartTime))
                    .add_column(date_null(ExtraTrip::PreparationSkipReturnDate))
                    .add_column(time_null(ExtraTrip::PreparationSkipReturnTime))
                    .add_column(string_len_null(ExtraTrip::PreparationSkipDestination, 200))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ExtraTrip::Table)
                    .drop_column(ExtraTrip::PreparationSkipAuthorizedBy)
                    .drop_column(ExtraTrip::PreparationSkipAuthorizedAt)
                    .drop_column(ExtraTrip::PreparationSkipJustification)
                    .drop_column(ExtraTrip::PreparationSkipVehicleId)
                    .drop_column(ExtraTrip::PreparationSkipTripDate)
                    .drop_column(ExtraTrip::PreparationSkipStartTime)
                    .drop_column(ExtraTrip::PreparationSkipReturnDate)
                    .drop_column(ExtraTrip::PreparationSkipReturnTime)
                    .drop_column(ExtraTrip::PreparationSkipDestination)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ExtraTrip {
    Table,
    PreparationSkipAuthorizedBy,
    PreparationSkipAuthorizedAt,
    PreparationSkipJustification,
    PreparationSkipVehicleId,
    PreparationSkipTripDate,
    PreparationSkipStartTime,
    PreparationSkipReturnDate,
    PreparationSkipReturnTime,
    PreparationSkipDestination,
}