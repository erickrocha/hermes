use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-SC-02-S01` (`HRMS-603`, `C-024`): recurring transport demand.
/// `customer_id`, `specific_driver_id` (a `user`) and `specific_vehicle_id`
/// are all nullable FKs -- a demand may name none, some, or all of them.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TransportDemand::Table)
                    .if_not_exists()
                    .col(pk_auto(TransportDemand::Id).integer())
                    .col(binary_len_uniq(TransportDemand::Uuid, 16))
                    .col(integer(TransportDemand::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_transport_demand_tenant")
                            .from(TransportDemand::Table, TransportDemand::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(string_len(TransportDemand::DemandType, 50).not_null())
                    .col(integer_null(TransportDemand::CustomerId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_transport_demand_customer")
                            .from(TransportDemand::Table, TransportDemand::CustomerId)
                            .to(Customer::Table, Customer::Id),
                    )
                    .col(string_len_null(TransportDemand::LineName, 200))
                    .col(time_null(TransportDemand::ShiftStart))
                    .col(time_null(TransportDemand::ShiftEnd))
                    .col(string_len_null(TransportDemand::DaysOfWeek, 50))
                    .col(date_null(TransportDemand::SpecificDate))
                    .col(integer_null(TransportDemand::Priority))
                    .col(string_len_null(TransportDemand::PreferredVehicleType, 100))
                    .col(string_len_null(TransportDemand::PreferredVehicleModel, 200))
                    .col(integer_null(TransportDemand::SpecificDriverId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_transport_demand_driver")
                            .from(TransportDemand::Table, TransportDemand::SpecificDriverId)
                            .to(User::Table, User::Id),
                    )
                    .col(integer_null(TransportDemand::SpecificVehicleId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_transport_demand_vehicle")
                            .from(TransportDemand::Table, TransportDemand::SpecificVehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(boolean(TransportDemand::Active).not_null().default(true))
                    .col(date_time(TransportDemand::CreatedAt).null())
                    .col(string_len(TransportDemand::CreatedBy, 50).null())
                    .col(date_time(TransportDemand::UpdatedAt).null())
                    .col(string_len(TransportDemand::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TransportDemand::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum TransportDemand {
    Table,
    Id,
    Uuid,
    TenantId,
    DemandType,
    CustomerId,
    LineName,
    ShiftStart,
    ShiftEnd,
    DaysOfWeek,
    SpecificDate,
    Priority,
    PreferredVehicleType,
    PreferredVehicleModel,
    SpecificDriverId,
    SpecificVehicleId,
    Active,
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
