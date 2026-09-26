use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-FO-06-S01` (`HRMS-941`, `C-023`): the operacao-trm parity fields
/// `entity-inventory.md` §1 names for `veiculos` and hermes' `vehicle` table
/// lacks -- `prefixo`, `tipo_veiculo`, `km_atual` (cache), the wheel/spare
/// fields, and the garage tag + its origin. All nullable: none of these
/// carry an owner-ruled vocabulary requiring a `NOT NULL` default, and
/// `odometer_km`/`garage_tag*` have no writer in hermes yet (`C-025`,
/// `C-028`).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Vehicle::Table)
                    .add_column(string_len_null(Vehicle::Prefix, 32))
                    .add_column(string_len_null(Vehicle::VehicleType, 100))
                    .add_column(double_null(Vehicle::OdometerKm))
                    .add_column(string_len_null(Vehicle::WheelType, 100))
                    .add_column(integer_null(Vehicle::SpareTireCount))
                    .add_column(string_len_null(Vehicle::SpareTireType, 100))
                    .add_column(string_len_null(Vehicle::SpareTireNotes, 500))
                    .add_column(string_len_null(Vehicle::GarageTag, 200))
                    .add_column(string_len_null(Vehicle::GarageTagOrigin, 50))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Vehicle::Table)
                    .drop_column(Vehicle::Prefix)
                    .drop_column(Vehicle::VehicleType)
                    .drop_column(Vehicle::OdometerKm)
                    .drop_column(Vehicle::WheelType)
                    .drop_column(Vehicle::SpareTireCount)
                    .drop_column(Vehicle::SpareTireType)
                    .drop_column(Vehicle::SpareTireNotes)
                    .drop_column(Vehicle::GarageTag)
                    .drop_column(Vehicle::GarageTagOrigin)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
// `VehicleType` must keep the `Vehicle` prefix: `DeriveIden` lowers it to the
// `vehicle_type` column name the entity actually uses.
#[allow(clippy::enum_variant_names)]
enum Vehicle {
    Table,
    Prefix,
    VehicleType,
    OdometerKm,
    WheelType,
    SpareTireCount,
    SpareTireType,
    SpareTireNotes,
    GarageTag,
    GarageTagOrigin,
}
