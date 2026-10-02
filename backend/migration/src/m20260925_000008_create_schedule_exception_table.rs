use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-SC-02-S04` (`HRMS-606`, `C-024`): a day exception against a demand.
/// `extra_trip_id` deliberately carries no foreign key -- `EPIC-SC-03`'s
/// trip table does not exist yet. Add the constraint when it does.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ScheduleException::Table)
                    .if_not_exists()
                    .col(pk_auto(ScheduleException::Id).integer())
                    .col(binary_len_uniq(ScheduleException::Uuid, 16))
                    .col(integer(ScheduleException::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_schedule_exception_tenant")
                            .from(ScheduleException::Table, ScheduleException::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(ScheduleException::DemandId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_schedule_exception_demand")
                            .from(ScheduleException::Table, ScheduleException::DemandId)
                            .to(TransportDemand::Table, TransportDemand::Id),
                    )
                    .col(date(ScheduleException::Date).not_null())
                    .col(string_len(ScheduleException::ExceptionType, 50).not_null())
                    .col(integer_null(ScheduleException::NewDriverId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_schedule_exception_new_driver")
                            .from(ScheduleException::Table, ScheduleException::NewDriverId)
                            .to(User::Table, User::Id),
                    )
                    .col(integer_null(ScheduleException::NewVehicleId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_schedule_exception_new_vehicle")
                            .from(ScheduleException::Table, ScheduleException::NewVehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(string_len_null(ScheduleException::Reason, 500))
                    // No FK: EPIC-SC-03 (viagensExtra) is not built yet.
                    .col(integer_null(ScheduleException::ExtraTripId))
                    .col(string_len_null(ScheduleException::Status, 50))
                    .col(date_time(ScheduleException::CreatedAt).null())
                    .col(string_len(ScheduleException::CreatedBy, 50).null())
                    .col(date_time(ScheduleException::UpdatedAt).null())
                    .col(string_len(ScheduleException::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ScheduleException::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum ScheduleException {
    Table,
    Id,
    Uuid,
    TenantId,
    DemandId,
    Date,
    ExceptionType,
    NewDriverId,
    NewVehicleId,
    Reason,
    ExtraTripId,
    Status,
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
