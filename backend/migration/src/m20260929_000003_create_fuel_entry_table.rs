use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-FU-01-S01` (`HRMS-942`, `C-029`): the fuel ledger. No provider
/// fields yet -- see `entity::fuel_entry_entity`'s doc comment.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FuelEntry::Table)
                    .if_not_exists()
                    .col(pk_auto(FuelEntry::Id).integer())
                    .col(binary_len_uniq(FuelEntry::Uuid, 16))
                    .col(integer(FuelEntry::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_fuel_entry_tenant")
                            .from(FuelEntry::Table, FuelEntry::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(FuelEntry::VehicleId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_fuel_entry_vehicle")
                            .from(FuelEntry::Table, FuelEntry::VehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(date_time(FuelEntry::RecordedAt).not_null())
                    .col(double(FuelEntry::VolumeLiters).not_null())
                    .col(big_integer(FuelEntry::ValueCents).not_null())
                    .col(double_null(FuelEntry::OdometerKm))
                    .col(string_len_null(FuelEntry::Station, 255))
                    .col(boolean(FuelEntry::FullTank).not_null())
                    .col(string_len(FuelEntry::Origin, 20).not_null())
                    .col(date_time(FuelEntry::CreatedAt).null())
                    .col(string_len(FuelEntry::CreatedBy, 50).null())
                    .col(date_time(FuelEntry::UpdatedAt).null())
                    .col(string_len(FuelEntry::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_fuel_entry_vehicle")
                    .table(FuelEntry::Table)
                    .col(FuelEntry::VehicleId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(FuelEntry::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum FuelEntry {
    Table,
    Id,
    Uuid,
    TenantId,
    VehicleId,
    RecordedAt,
    VolumeLiters,
    ValueCents,
    OdometerKm,
    Station,
    FullTank,
    Origin,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum Vehicle {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Tenant {
    Table,
    Id,
}
