use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-MT-06` (`C-026`, `TRM-616`): in-app notices that a part a work order
/// was waiting for has arrived. `part_name` is copied when the notice is
/// raised, so renaming the part later does not rewrite history.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MaintenanceNotification::Table)
                    .if_not_exists()
                    .col(pk_auto(MaintenanceNotification::Id).integer())
                    .col(binary_len_uniq(MaintenanceNotification::Uuid, 16))
                    .col(integer(MaintenanceNotification::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_maintenance_notification_tenant")
                            .from(
                                MaintenanceNotification::Table,
                                MaintenanceNotification::TenantId,
                            )
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(MaintenanceNotification::WorkOrderId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_maintenance_notification_work_order")
                            .from(
                                MaintenanceNotification::Table,
                                MaintenanceNotification::WorkOrderId,
                            )
                            .to(WorkOrder::Table, WorkOrder::Id),
                    )
                    .col(big_integer(MaintenanceNotification::PartId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_maintenance_notification_part")
                            .from(
                                MaintenanceNotification::Table,
                                MaintenanceNotification::PartId,
                            )
                            .to(Part::Table, Part::Id),
                    )
                    .col(string_len(MaintenanceNotification::PartName, 255).not_null())
                    .col(double(MaintenanceNotification::Quantity).not_null())
                    .col(date_time(MaintenanceNotification::ReadAt).null())
                    .col(date_time(MaintenanceNotification::CreatedAt).null())
                    .col(string_len(MaintenanceNotification::CreatedBy, 50).null())
                    .col(date_time(MaintenanceNotification::UpdatedAt).null())
                    .col(string_len(MaintenanceNotification::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_maintenance_notification_tenant_read")
                    .table(MaintenanceNotification::Table)
                    .col(MaintenanceNotification::TenantId)
                    .col(MaintenanceNotification::ReadAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(MaintenanceNotification::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
pub enum MaintenanceNotification {
    Table,
    Id,
    Uuid,
    TenantId,
    WorkOrderId,
    PartId,
    PartName,
    Quantity,
    ReadAt,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum Part {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum WorkOrder {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Tenant {
    Table,
    Id,
}
