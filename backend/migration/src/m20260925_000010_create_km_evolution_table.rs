use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-CK-01-S01` (`HRMS-650`, `C-025`, `AD-041`): the one official
/// odometer writer. `source_entity_id` carries no foreign key -- see the
/// entity's own doc comment for why.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(KmEvolution::Table)
                    .if_not_exists()
                    .col(pk_auto(KmEvolution::Id).integer())
                    .col(binary_len_uniq(KmEvolution::Uuid, 16))
                    .col(integer(KmEvolution::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_km_evolution_tenant")
                            .from(KmEvolution::Table, KmEvolution::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(KmEvolution::VehicleId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_km_evolution_vehicle")
                            .from(KmEvolution::Table, KmEvolution::VehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(double(KmEvolution::Km).not_null())
                    .col(date_time(KmEvolution::RecordedAt).not_null())
                    .col(string_len(KmEvolution::Origin, 50).not_null())
                    .col(string_len_null(KmEvolution::SourceEntity, 50))
                    .col(integer_null(KmEvolution::SourceEntityId))
                    .col(string_len_null(KmEvolution::Notes, 500))
                    .col(integer_null(KmEvolution::RecordedByUserId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_km_evolution_recorded_by")
                            .from(KmEvolution::Table, KmEvolution::RecordedByUserId)
                            .to(User::Table, User::Id),
                    )
                    .col(date_time(KmEvolution::CreatedAt).null())
                    .col(string_len(KmEvolution::CreatedBy, 50).null())
                    .col(date_time(KmEvolution::UpdatedAt).null())
                    .col(string_len(KmEvolution::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;

        // Every read of a vehicle's history is newest-first; this is the
        // access pattern `find_page_by_vehicle` and the odometer
        // recomputation both use.
        manager
            .create_index(
                Index::create()
                    .name("idx_km_evolution_vehicle_recorded_at")
                    .table(KmEvolution::Table)
                    .col(KmEvolution::VehicleId)
                    .col(KmEvolution::RecordedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(KmEvolution::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum KmEvolution {
    Table,
    Id,
    Uuid,
    TenantId,
    VehicleId,
    Km,
    RecordedAt,
    Origin,
    SourceEntity,
    SourceEntityId,
    Notes,
    RecordedByUserId,
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
