use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Setting::Table)
                    .add_column(
                        double(Setting::GarageCycleFallbackDistanceKm)
                            .not_null()
                            .default(2.0),
                    )
                    .add_column(
                        double(Setting::GarageOdometerMaxSingleJumpKm)
                            .not_null()
                            .default(300.0),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Setting::Table)
                    .drop_column(Setting::GarageCycleFallbackDistanceKm)
                    .drop_column(Setting::GarageOdometerMaxSingleJumpKm)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Setting {
    #[sea_orm(iden = "tenant_rule_setting")]
    Table,
    GarageCycleFallbackDistanceKm,
    GarageOdometerMaxSingleJumpKm,
}
