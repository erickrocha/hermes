use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Attendance::Table)
                    .add_column(text(Attendance::AutomaticDepartureJustification).null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Attendance::Table)
                    .drop_column(Attendance::AutomaticDepartureJustification)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Attendance {
    #[sea_orm(iden = "garage_attendance")]
    Table,
    AutomaticDepartureJustification,
}
