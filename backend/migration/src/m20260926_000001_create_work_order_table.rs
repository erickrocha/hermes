use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-MT-01-S01` (`HRMS-700`, `C-026`): a maintenance work order. No
/// `numero_os` counter column -- see this Change's plan doc for why the
/// `id` itself is the human-facing number.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(WorkOrder::Table)
                    .if_not_exists()
                    .col(pk_auto(WorkOrder::Id).integer())
                    .col(binary_len_uniq(WorkOrder::Uuid, 16))
                    .col(integer(WorkOrder::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_work_order_tenant")
                            .from(WorkOrder::Table, WorkOrder::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(WorkOrder::VehicleId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_work_order_vehicle")
                            .from(WorkOrder::Table, WorkOrder::VehicleId)
                            .to(Vehicle::Table, Vehicle::Id),
                    )
                    .col(date_time(WorkOrder::OpenedAt).not_null())
                    .col(double(WorkOrder::OdometerKm).not_null())
                    .col(string_len(WorkOrder::Origin, 50).not_null())
                    .col(string_len_null(WorkOrder::ServiceType, 255))
                    .col(string_len(WorkOrder::Description, 500).not_null())
                    .col(string_len_null(WorkOrder::Responsible, 255))
                    .col(string_len(WorkOrder::Status, 50).not_null())
                    .col(string_len_null(WorkOrder::Observation, 500))
                    .col(boolean(WorkOrder::ExternalService).not_null())
                    .col(string_len_null(WorkOrder::Supplier, 255))
                    .col(string_len_null(WorkOrder::InvoiceNumber, 100))
                    .col(big_integer_null(WorkOrder::InvoiceValueCents))
                    .col(date_null(WorkOrder::InvoiceDate))
                    .col(date_null(WorkOrder::ConcludedAt))
                    .col(date_time(WorkOrder::CreatedAt).null())
                    .col(string_len(WorkOrder::CreatedBy, 50).null())
                    .col(date_time(WorkOrder::UpdatedAt).null())
                    .col(string_len(WorkOrder::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_work_order_vehicle_status")
                    .table(WorkOrder::Table)
                    .col(WorkOrder::VehicleId)
                    .col(WorkOrder::Status)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(WorkOrder::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum WorkOrder {
    Table,
    Id,
    Uuid,
    TenantId,
    VehicleId,
    OpenedAt,
    OdometerKm,
    Origin,
    ServiceType,
    Description,
    Responsible,
    Status,
    Observation,
    ExternalService,
    Supplier,
    InvoiceNumber,
    InvoiceValueCents,
    InvoiceDate,
    ConcludedAt,
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
