use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-SC-02-S02` (`HRMS-604`, `C-024`): a demand's crew (driver +
/// vehicle) over a date range.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TransportDemandAllocation::Table)
                    .if_not_exists()
                    .col(pk_auto(TransportDemandAllocation::Id).integer())
                    .col(binary_len_uniq(TransportDemandAllocation::Uuid, 16))
                    .col(integer(TransportDemandAllocation::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_transport_demand_allocation_tenant")
                            .from(TransportDemandAllocation::Table, TransportDemandAllocation::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(TransportDemandAllocation::DemandId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_transport_demand_allocation_demand")
                            .from(TransportDemandAllocation::Table, TransportDemandAllocation::DemandId)
                            .to(TransportDemand::Table, TransportDemand::Id),
                    )
                    .col(integer(TransportDemandAllocation::DriverId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_transport_demand_allocation_driver")
                            .from(TransportDemandAllocation::Table, TransportDemandAllocation::DriverId)
                            .to(User::Table, User::Id),
                    )
                    .col(integer(TransportDemandAllocation::VehicleId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_transport_demand_allocation_vehicle")
                            .from(TransportDemandAllocation::Table, TransportDemandAllocation::VehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(string_len_null(TransportDemandAllocation::DaysOfWeek, 50))
                    .col(date(TransportDemandAllocation::StartDate).not_null())
                    .col(date_null(TransportDemandAllocation::EndDate))
                    .col(boolean(TransportDemandAllocation::Active).not_null().default(true))
                    .col(date_time(TransportDemandAllocation::CreatedAt).null())
                    .col(string_len(TransportDemandAllocation::CreatedBy, 50).null())
                    .col(date_time(TransportDemandAllocation::UpdatedAt).null())
                    .col(string_len(TransportDemandAllocation::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TransportDemandAllocation::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum TransportDemandAllocation {
    Table,
    Id,
    Uuid,
    TenantId,
    DemandId,
    DriverId,
    VehicleId,
    DaysOfWeek,
    StartDate,
    EndDate,
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
