use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `D-21`/`PD-042`: each plan has one overage price per enabled human user and cycle.
/// Existing plans start at zero so this migration never creates retroactive charges.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(BusinessPlan::Table)
                    .add_column(
                        big_integer(BusinessPlan::PricePerExtraUserInCents)
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(BusinessPlan::Table)
                    .drop_column(BusinessPlan::PricePerExtraUserInCents)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum BusinessPlan {
    Table,
    PricePerExtraUserInCents,
}