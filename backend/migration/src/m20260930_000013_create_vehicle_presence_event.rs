use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-GA-03-S01` (`HRMS-960`, `C-028`): the vehicle's physical arrivals and
/// departures, one row per event (`TRM-770…773`). Events, not just the latest
/// stamps, because the validity rule also asks about completed runs
/// (`TRM-466`). Written only by the presence domain (`TRM-788`).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Event::Table)
                    .if_not_exists()
                    .col(pk_auto(Event::Id).integer())
                    .col(binary_len_uniq(Event::Uuid, 16))
                    .col(integer(Event::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_vehicle_presence_event_tenant")
                            .from(Event::Table, Event::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(Event::VehicleId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_vehicle_presence_event_vehicle")
                            .from(Event::Table, Event::VehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(string_len(Event::Kind, 20).not_null())
                    .col(date_time(Event::OccurredAt).not_null())
                    .col(date_time(Event::CreatedAt).null())
                    .col(string_len(Event::CreatedBy, 50).null())
                    .col(date_time(Event::UpdatedAt).null())
                    .col(string_len(Event::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_vehicle_presence_event_vehicle_at")
                    .table(Event::Table)
                    .col(Event::VehicleId)
                    .col(Event::OccurredAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Event::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Event {
    #[sea_orm(iden = "vehicle_presence_event")]
    Table,
    Id,
    Uuid,
    TenantId,
    VehicleId,
    Kind,
    OccurredAt,
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
