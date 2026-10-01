use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-GA-07-S01` (`HRMS-964`, `C-028`): the monitor's three `[TC]` windows --
/// urgent animation 90 min (`TRM-498`), readiness alert 120 min for a trip and
/// 30 min for a line (`TRM-499`).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Setting::Table)
                    .add_column(integer(Setting::GarageMonitorUrgentMinutes).not_null().default(90))
                    .add_column(integer(Setting::GarageMonitorTripWindowMinutes).not_null().default(120))
                    .add_column(integer(Setting::GarageMonitorLineWindowMinutes).not_null().default(30))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Setting::Table)
                    .drop_column(Setting::GarageMonitorUrgentMinutes)
                    .drop_column(Setting::GarageMonitorTripWindowMinutes)
                    .drop_column(Setting::GarageMonitorLineWindowMinutes)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Setting {
    #[sea_orm(iden = "tenant_rule_setting")]
    Table,
    GarageMonitorUrgentMinutes,
    GarageMonitorTripWindowMinutes,
    GarageMonitorLineWindowMinutes,
}
