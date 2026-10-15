use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-GA-02-S01`/`S02` (`HRMS-958`/`959`, `C-028`): the triage, its service
/// records and the independent marking log. `garage_attendance.active_marker`
/// is 1 while the triage is active and NULL once terminal, so the unique
/// (vehicle, marker) index allows at most one active triage per vehicle
/// (`TRM-412`) while any number of closed ones; `garage_service` is unique per
/// (triage, normalised name) (`TRM-434`).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Attendance::Table)
                    .if_not_exists()
                    .col(pk_auto(Attendance::Id).integer())
                    .col(binary_len_uniq(Attendance::Uuid, 16))
                    .col(integer(Attendance::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_attendance_tenant")
                            .from(Attendance::Table, Attendance::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(Attendance::VehicleId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_attendance_vehicle_id")
                            .from(Attendance::Table, Attendance::VehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(date(Attendance::AttendanceDate).not_null())
                    .col(date_time(Attendance::CheckedInAt).not_null())
                    .col(string_len(Attendance::Status, 30).not_null())
                    .col(integer_null(Attendance::ManualPriority))
                    .col(date_time_null(Attendance::ReleasedAt))
                    .col(string_len(Attendance::Origin, 30).not_null())
                    .col(integer_null(Attendance::ActiveMarker))
                    .col(date_time(Attendance::CreatedAt).null())
                    .col(string_len(Attendance::CreatedBy, 50).null())
                    .col(date_time(Attendance::UpdatedAt).null())
                    .col(string_len(Attendance::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Service::Table)
                    .if_not_exists()
                    .col(pk_auto(Service::Id).integer())
                    .col(binary_len_uniq(Service::Uuid, 16))
                    .col(integer(Service::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_service_tenant")
                            .from(Service::Table, Service::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(Service::AttendanceId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_service_attendance_id")
                            .from(Service::Table, Service::AttendanceId)
                            .to(GarageAttendance::Table, GarageAttendance::Id),
                    )
                    .col(integer(Service::ServiceModelId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_service_service_model_id")
                            .from(Service::Table, Service::ServiceModelId)
                            .to(GarageServiceModel::Table, GarageServiceModel::Id),
                    )
                    .col(string_len(Service::NameKey, 120).not_null())
                    .col(string_len(Service::State, 30).not_null())
                    .col(date_time_null(Service::PerformedAt))
                    .col(date_time_null(Service::MarkedAt))
                    .col(date_time_null(Service::ForcedPendingAt))
                    .col(date_time(Service::CreatedAt).null())
                    .col(string_len(Service::CreatedBy, 50).null())
                    .col(date_time(Service::UpdatedAt).null())
                    .col(string_len(Service::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Log::Table)
                    .if_not_exists()
                    .col(pk_auto(Log::Id).integer())
                    .col(binary_len_uniq(Log::Uuid, 16))
                    .col(integer(Log::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_service_log_tenant")
                            .from(Log::Table, Log::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(Log::AttendanceId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_service_log_attendance_id")
                            .from(Log::Table, Log::AttendanceId)
                            .to(GarageAttendance::Table, GarageAttendance::Id),
                    )
                    .col(integer(Log::VehicleId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_service_log_vehicle_id")
                            .from(Log::Table, Log::VehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(integer(Log::ServiceModelId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_service_log_service_model_id")
                            .from(Log::Table, Log::ServiceModelId)
                            .to(GarageServiceModel::Table, GarageServiceModel::Id),
                    )
                    .col(string_len(Log::NameKey, 120).not_null())
                    .col(string_len(Log::NewState, 30).not_null())
                    .col(integer_null(Log::ActedByUserId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_garage_service_log_acted_by_user_id")
                            .from(Log::Table, Log::ActedByUserId)
                            .to(User::Table, User::Id),
                    )
                    .col(date_time(Log::ActedAt).not_null())
                    .col(string_len(Log::Origin, 30).not_null())
                    .col(date_time(Log::CreatedAt).null())
                    .col(string_len(Log::CreatedBy, 50).null())
                    .col(date_time(Log::UpdatedAt).null())
                    .col(string_len(Log::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_garage_attendance_active_vehicle")
                    .table(Attendance::Table)
                    .col(Attendance::VehicleId)
                    .col(Attendance::ActiveMarker)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_garage_service_attendance_name")
                    .table(Service::Table)
                    .col(Service::AttendanceId)
                    .col(Service::NameKey)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Log::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Service::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Attendance::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
#[allow(clippy::enum_variant_names)]
enum Attendance {
    #[sea_orm(iden = "garage_attendance")]
    Table,
    Id,
    Uuid,
    TenantId,
    VehicleId,
    AttendanceDate,
    CheckedInAt,
    Status,
    ManualPriority,
    ReleasedAt,
    Origin,
    ActiveMarker,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
#[allow(clippy::enum_variant_names)]
enum Service {
    #[sea_orm(iden = "garage_service")]
    Table,
    Id,
    Uuid,
    TenantId,
    AttendanceId,
    ServiceModelId,
    NameKey,
    State,
    PerformedAt,
    MarkedAt,
    ForcedPendingAt,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum Log {
    #[sea_orm(iden = "garage_service_log")]
    Table,
    Id,
    Uuid,
    TenantId,
    AttendanceId,
    VehicleId,
    ServiceModelId,
    NameKey,
    NewState,
    ActedByUserId,
    ActedAt,
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
enum User {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum GarageServiceModel {
    #[sea_orm(iden = "garage_service_model")]
    Table,
    Id,
}

#[derive(DeriveIden)]
enum GarageAttendance {
    #[sea_orm(iden = "garage_attendance")]
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Tenant {
    Table,
    Id,
}
