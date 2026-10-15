use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Cycle::Table)
                    .if_not_exists()
                    .col(pk_auto(Cycle::Id).integer())
                    .col(binary_len_uniq(Cycle::Uuid, 16))
                    .col(integer(Cycle::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_cycle_state_tenant")
                            .from(Cycle::Table, Cycle::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(Cycle::VehicleId).not_null().unique_key())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_cycle_state_vehicle")
                            .from(Cycle::Table, Cycle::VehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(double(Cycle::ReferenceOdometerMeters).null())
                    .col(double(Cycle::LastObservedOdometerMeters).null())
                    .col(date_time(Cycle::LastObservedAt).null())
                    .col(date_time(Cycle::OpenCycleStartedAt).null())
                    .col(double(Cycle::OpenCycleStartOdometerMeters).null())
                    .col(
                        double(Cycle::OpenCycleDistanceMeters)
                            .not_null()
                            .default(0.0),
                    )
                    .col(double(Cycle::LastCompletedCycleDistanceMeters).null())
                    .col(date_time(Cycle::LastCompletedCycleAt).null())
                    .col(date_time(Cycle::CreatedAt).null())
                    .col(string_len(Cycle::CreatedBy, 50).null())
                    .col(date_time(Cycle::UpdatedAt).null())
                    .col(string_len(Cycle::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Cycle::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Cycle {
    #[sea_orm(iden = "garage_cycle_state")]
    Table,
    Id,
    Uuid,
    TenantId,
    VehicleId,
    ReferenceOdometerMeters,
    LastObservedOdometerMeters,
    LastObservedAt,
    OpenCycleStartedAt,
    OpenCycleStartOdometerMeters,
    OpenCycleDistanceMeters,
    LastCompletedCycleDistanceMeters,
    LastCompletedCycleAt,
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
