use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-FU-06-S04` (`HRMS-955`, `TRM-569`/`570`): a vehicle's registered
/// reference consumption (tenant data), and the tenant's peer-fallback
/// thresholds.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter().table(Vehicle::Table).add_column(double_null(Vehicle::ReferenceKmPerLiter)).to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Setting::Table)
                    .add_column(double(Setting::AvgPeerCapacityTolerance).not_null().default(0.3))
                    .add_column(integer(Setting::AvgPeerMinCount).not_null().default(2))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Setting::Table)
                    .drop_column(Setting::AvgPeerCapacityTolerance)
                    .drop_column(Setting::AvgPeerMinCount)
                    .to_owned(),
            )
            .await?;
        manager.alter_table(Table::alter().table(Vehicle::Table).drop_column(Vehicle::ReferenceKmPerLiter).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Vehicle {
    Table,
    ReferenceKmPerLiter,
}

#[derive(DeriveIden)]
enum Setting {
    #[sea_orm(iden = "tenant_rule_setting")]
    Table,
    AvgPeerCapacityTolerance,
    AvgPeerMinCount,
}
