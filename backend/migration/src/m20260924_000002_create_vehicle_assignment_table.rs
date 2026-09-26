use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// EPIC-FO-03 (HRMS-931…934): who is responsible for which vehicle, and since
/// when. An assignment is ended, never deleted, so the history stays.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(VehicleAssignment::Table)
                    .if_not_exists()
                    .col(pk_auto(VehicleAssignment::Id).integer())
                    .col(binary_len_uniq(VehicleAssignment::Uuid, 16))
                    // D-09: tenant-owned, NOT NULL at the schema level.
                    .col(integer(VehicleAssignment::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_vehicle_assignment_tenant")
                            .from(VehicleAssignment::Table, VehicleAssignment::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(VehicleAssignment::VehicleId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_vehicle_assignment_vehicle")
                            .from(VehicleAssignment::Table, VehicleAssignment::VehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(integer(VehicleAssignment::DriverId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_vehicle_assignment_driver")
                            .from(VehicleAssignment::Table, VehicleAssignment::DriverId)
                            .to(User::Table, User::Id),
                    )
                    .col(date_time(VehicleAssignment::StartedAt).not_null())
                    .col(date_time(VehicleAssignment::EndedAt).null())
                    .col(date_time(VehicleAssignment::CreatedAt).null())
                    .col(string_len(VehicleAssignment::CreatedBy, 50).null())
                    .col(date_time(VehicleAssignment::UpdatedAt).null())
                    .col(string_len(VehicleAssignment::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;

        // D-23(d): at most one live assignment per vehicle, enforced by the
        // database so two concurrent assignments cannot both win. The
        // generated column holds the vehicle id only while the assignment is
        // live; a unique index ignores NULLs, so ended rows never collide.
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE vehicle_assignment \
                 ADD COLUMN live_vehicle_id INT \
                 AS (CASE WHEN ended_at IS NULL THEN vehicle_id END) PERSISTENT, \
                 ADD UNIQUE INDEX uq_vehicle_assignment_live (live_vehicle_id)",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(VehicleAssignment::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum VehicleAssignment {
    Table,
    Id,
    Uuid,
    TenantId,
    VehicleId,
    DriverId,
    StartedAt,
    EndedAt,
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
enum Vehicle {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum User {
    Table,
    Id,
}
