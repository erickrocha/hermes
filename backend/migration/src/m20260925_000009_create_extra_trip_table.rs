use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-SC-03-S01` (`HRMS-607`, `C-024`, `D-24(f)`): the canonical Trip.
/// Identity is `order_code` + `trip_date` (`D-24(d)`), not code alone.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ExtraTrip::Table)
                    .if_not_exists()
                    .col(pk_auto(ExtraTrip::Id).integer())
                    .col(binary_len_uniq(ExtraTrip::Uuid, 16))
                    .col(integer(ExtraTrip::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_extra_trip_tenant")
                            .from(ExtraTrip::Table, ExtraTrip::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(string_len(ExtraTrip::OrderCode, 100).not_null())
                    .col(date(ExtraTrip::TripDate).not_null())
                    .col(integer_null(ExtraTrip::CustomerId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_extra_trip_customer")
                            .from(ExtraTrip::Table, ExtraTrip::CustomerId)
                            .to(Customer::Table, Customer::Id),
                    )
                    .col(time_null(ExtraTrip::StartTime))
                    .col(date_null(ExtraTrip::ReturnDate))
                    .col(time_null(ExtraTrip::ReturnTime))
                    .col(string_len_null(ExtraTrip::Destination, 200))
                    .col(string_len_null(ExtraTrip::OriginCity, 200))
                    .col(string_len_null(ExtraTrip::Stops, 1000))
                    .col(string_len_null(ExtraTrip::PreferredVehicleType, 100))
                    .col(integer_null(ExtraTrip::DriverId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_extra_trip_driver")
                            .from(ExtraTrip::Table, ExtraTrip::DriverId)
                            .to(User::Table, User::Id),
                    )
                    .col(integer_null(ExtraTrip::SecondDriverId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_extra_trip_second_driver")
                            .from(ExtraTrip::Table, ExtraTrip::SecondDriverId)
                            .to(User::Table, User::Id),
                    )
                    .col(integer_null(ExtraTrip::VehicleId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_extra_trip_vehicle")
                            .from(ExtraTrip::Table, ExtraTrip::VehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(big_integer_null(ExtraTrip::FreightValueCents))
                    .col(string_len_null(ExtraTrip::Payment, 100))
                    .col(string_len(ExtraTrip::Status, 50).not_null())
                    .col(string_len_null(ExtraTrip::Origin, 50))
                    .col(string_len_null(ExtraTrip::ImportBatchId, 100))
                    .col(date_time_null(ExtraTrip::ImportedAt))
                    .col(integer_null(ExtraTrip::ReplacedById))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_extra_trip_replaced_by")
                            .from(ExtraTrip::Table, ExtraTrip::ReplacedById)
                            .to(ExtraTrip::Table, ExtraTrip::Id),
                    )
                    .col(date_time(ExtraTrip::CreatedAt).null())
                    .col(string_len(ExtraTrip::CreatedBy, 50).null())
                    .col(date_time(ExtraTrip::UpdatedAt).null())
                    .col(string_len(ExtraTrip::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;

        // D-24(d): a trip's identity is order code + date, not code alone --
        // the source system reuses codes on different dates.
        manager
            .create_index(
                Index::create()
                    .name("uq_extra_trip_tenant_code_date")
                    .table(ExtraTrip::Table)
                    .col(ExtraTrip::TenantId)
                    .col(ExtraTrip::OrderCode)
                    .col(ExtraTrip::TripDate)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ExtraTrip::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum ExtraTrip {
    Table,
    Id,
    Uuid,
    TenantId,
    OrderCode,
    TripDate,
    CustomerId,
    StartTime,
    ReturnDate,
    ReturnTime,
    Destination,
    OriginCity,
    Stops,
    PreferredVehicleType,
    DriverId,
    SecondDriverId,
    VehicleId,
    FreightValueCents,
    Payment,
    Status,
    Origin,
    ImportBatchId,
    ImportedAt,
    ReplacedById,
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
enum Customer {
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
