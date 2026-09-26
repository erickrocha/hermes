use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// EPIC-FO-02-S01 (HRMS-926): the tracking provider's device id, so a
/// position hermes already receives belongs to a vehicle someone recognises.
///
/// Unique across the platform, not per tenant: there is one provider account
/// for the whole platform, so a device reports for exactly one vehicle
/// anywhere. Per-tenant uniqueness would let a second customer claim the same
/// device and read the first customer's vehicle position (HRMS-927). Only the
/// platform administrator sets it, so the 409 discloses nothing.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Vehicle::Table)
                    .add_column(big_integer_null(Vehicle::TrackerDeviceId))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_vehicle_tracker_device")
                    .table(Vehicle::Table)
                    .col(Vehicle::TrackerDeviceId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Dropping the column drops its index with it.
        manager
            .alter_table(
                Table::alter()
                    .table(Vehicle::Table)
                    .drop_column(Vehicle::TrackerDeviceId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Vehicle {
    Table,
    TrackerDeviceId,
}
