use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-SC-02-S03` (`HRMS-605`, `C-024`): a manual one-day schedule entry.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(DailySchedule::Table)
                    .if_not_exists()
                    .col(pk_auto(DailySchedule::Id).integer())
                    .col(binary_len_uniq(DailySchedule::Uuid, 16))
                    .col(integer(DailySchedule::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_daily_schedule_tenant")
                            .from(DailySchedule::Table, DailySchedule::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(DailySchedule::DemandId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_daily_schedule_demand")
                            .from(DailySchedule::Table, DailySchedule::DemandId)
                            .to(TransportDemand::Table, TransportDemand::Id),
                    )
                    .col(integer(DailySchedule::DriverId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_daily_schedule_driver")
                            .from(DailySchedule::Table, DailySchedule::DriverId)
                            .to(User::Table, User::Id),
                    )
                    .col(integer(DailySchedule::VehicleId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_daily_schedule_vehicle")
                            .from(DailySchedule::Table, DailySchedule::VehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(date(DailySchedule::Date).not_null())
                    .col(time_null(DailySchedule::StartTime))
                    .col(time_null(DailySchedule::EndTime))
                    .col(string_len_null(DailySchedule::Notes, 500))
                    .col(date_time(DailySchedule::CreatedAt).null())
                    .col(string_len(DailySchedule::CreatedBy, 50).null())
                    .col(date_time(DailySchedule::UpdatedAt).null())
                    .col(string_len(DailySchedule::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;

        // At most one manual entry per demand per date -- a second entry for
        // the same day is a correction (delete and recreate), not a second
        // schedule.
        manager
            .create_index(
                Index::create()
                    .name("uq_daily_schedule_demand_date")
                    .table(DailySchedule::Table)
                    .col(DailySchedule::DemandId)
                    .col(DailySchedule::Date)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(DailySchedule::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum DailySchedule {
    Table,
    Id,
    Uuid,
    TenantId,
    DemandId,
    DriverId,
    VehicleId,
    Date,
    StartTime,
    EndTime,
    Notes,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum Tenant {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum TransportDemand {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum User {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Vehicle {
    Table,
    Id,
}
