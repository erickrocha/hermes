use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-FU-06-S01` (`HRMS-951`, `TRM-580`/`581`): a vehicle's registered tank
/// capacity (`tanque_litros`), the base of the tank gauge. Null = not
/// registered, which the gauge reports distinctly (`TRM-581`).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(Table::alter().table(Vehicle::Table).add_column(double_null(Vehicle::TankCapacityLiters)).to_owned())
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(Table::alter().table(Vehicle::Table).drop_column(Vehicle::TankCapacityLiters).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Vehicle {
    Table,
    TankCapacityLiters,
}
