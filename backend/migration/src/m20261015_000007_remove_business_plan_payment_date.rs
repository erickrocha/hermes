use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(BusinessPlan::Table)
                    .drop_column(BusinessPlan::PaymentDate)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(BusinessPlan::Table)
                    .add_column(date(BusinessPlan::PaymentDate).null())
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum BusinessPlan {
    Table,
    PaymentDate,
}