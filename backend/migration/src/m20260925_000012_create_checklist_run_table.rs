use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-CK-03-S01` (`HRMS-652`, `C-025`): a submitted driver checklist.
/// `opening_checklist_id` is a self-referencing FK, unique when present, so
/// at most one `Return` row may ever close a given `Departure` row.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ChecklistRun::Table)
                    .if_not_exists()
                    .col(pk_auto(ChecklistRun::Id).integer())
                    .col(binary_len_uniq(ChecklistRun::Uuid, 16))
                    .col(integer(ChecklistRun::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_checklist_run_tenant")
                            .from(ChecklistRun::Table, ChecklistRun::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(ChecklistRun::ChecklistTemplateId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_checklist_run_template")
                            .from(ChecklistRun::Table, ChecklistRun::ChecklistTemplateId)
                            .to(ChecklistTemplate::Table, ChecklistTemplate::Id),
                    )
                    .col(integer(ChecklistRun::DriverId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_checklist_run_driver")
                            .from(ChecklistRun::Table, ChecklistRun::DriverId)
                            .to(User::Table, User::Id),
                    )
                    .col(integer(ChecklistRun::VehicleId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_checklist_run_vehicle")
                            .from(ChecklistRun::Table, ChecklistRun::VehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(string_len(ChecklistRun::ChecklistType, 50).not_null())
                    .col(double(ChecklistRun::OdometerKm).not_null())
                    .col(string_len_null(ChecklistRun::Notes, 500))
                    .col(integer_null(ChecklistRun::OpeningChecklistId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_checklist_run_opening_checklist")
                            .from(ChecklistRun::Table, ChecklistRun::OpeningChecklistId)
                            .to(ChecklistRun::Table, ChecklistRun::Id),
                    )
                    .col(date_time(ChecklistRun::CreatedAt).null())
                    .col(string_len(ChecklistRun::CreatedBy, 50).null())
                    .col(date_time(ChecklistRun::UpdatedAt).null())
                    .col(string_len(ChecklistRun::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_checklist_run_vehicle_created_at")
                    .table(ChecklistRun::Table)
                    .col(ChecklistRun::VehicleId)
                    .col(ChecklistRun::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_checklist_run_driver_created_at")
                    .table(ChecklistRun::Table)
                    .col(ChecklistRun::DriverId)
                    .col(ChecklistRun::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_checklist_run_opening_checklist")
                    .table(ChecklistRun::Table)
                    .col(ChecklistRun::OpeningChecklistId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ChecklistRun::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum ChecklistRun {
    Table,
    Id,
    Uuid,
    TenantId,
    ChecklistTemplateId,
    DriverId,
    VehicleId,
    ChecklistType,
    OdometerKm,
    Notes,
    OpeningChecklistId,
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
enum ChecklistTemplate {
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
