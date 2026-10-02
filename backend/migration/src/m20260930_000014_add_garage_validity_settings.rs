use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-GA-04` (`HRMS-961`, `C-028`): the catalogue attribute that says a
/// service is governed by the tank (`TRM-462` -- never a name match, `TRM-470`'s
/// reasoning), and the tenant's validity thresholds (`[TC]`: 36 h, 1 h, 50 %,
/// 95 %).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Model::Table)
                    .add_column(boolean(Model::GovernedByTank).not_null().default(false))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Setting::Table)
                    .add_column(integer(Setting::GarageValidityHours).not_null().default(36))
                    .add_column(integer(Setting::GarageMinTripAbsenceMinutes).not_null().default(60))
                    .add_column(double(Setting::GarageFuelPendingBelowPercent).not_null().default(50.0))
                    .add_column(double(Setting::GarageTripFuelExemptPercent).not_null().default(95.0))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Setting::Table)
                    .drop_column(Setting::GarageValidityHours)
                    .drop_column(Setting::GarageMinTripAbsenceMinutes)
                    .drop_column(Setting::GarageFuelPendingBelowPercent)
                    .drop_column(Setting::GarageTripFuelExemptPercent)
                    .to_owned(),
            )
            .await?;
        manager.alter_table(Table::alter().table(Model::Table).drop_column(Model::GovernedByTank).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Model {
    #[sea_orm(iden = "garage_service_model")]
    Table,
    GovernedByTank,
}

#[derive(DeriveIden)]
#[allow(clippy::enum_variant_names)]
enum Setting {
    #[sea_orm(iden = "tenant_rule_setting")]
    Table,
    GarageValidityHours,
    GarageMinTripAbsenceMinutes,
    GarageFuelPendingBelowPercent,
    GarageTripFuelExemptPercent,
}
