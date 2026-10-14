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
                    .add_column(double(Setting::GarageBaseLatitude).null())
                    .add_column(double(Setting::GarageBaseLongitude).null())
                    .add_column(
                        double(Setting::GarageBaseRadiusMeters)
                            .not_null()
                            .default(75.0),
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
                    .drop_column(Setting::GarageBaseLatitude)
                    .drop_column(Setting::GarageBaseLongitude)
                    .drop_column(Setting::GarageBaseRadiusMeters)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Setting {
    #[sea_orm(iden = "tenant_rule_setting")]
    Table,
    GarageBaseLatitude,
    GarageBaseLongitude,
    GarageBaseRadiusMeters,
}
