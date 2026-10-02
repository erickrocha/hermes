use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-MT-01-S02` (`HRMS-701`, `C-026`): a work order's individually
/// tracked pendencies.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(WorkOrderItem::Table)
                    .if_not_exists()
                    .col(pk_auto(WorkOrderItem::Id).integer())
                    .col(binary_len_uniq(WorkOrderItem::Uuid, 16))
                    .col(integer(WorkOrderItem::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_work_order_item_tenant")
                            .from(WorkOrderItem::Table, WorkOrderItem::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(integer(WorkOrderItem::WorkOrderId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_work_order_item_work_order")
                            .from(WorkOrderItem::Table, WorkOrderItem::WorkOrderId)
                            .to(WorkOrder::Table, WorkOrder::Id),
                    )
                    .col(string_len(WorkOrderItem::Description, 500).not_null())
                    .col(string_len_null(WorkOrderItem::ItemType, 255))
                    .col(string_len(WorkOrderItem::Status, 50).not_null())
                    .col(string_len_null(WorkOrderItem::Observation, 500))
                    .col(string_len_null(WorkOrderItem::ResolvedBy, 255))
                    .col(date_time_null(WorkOrderItem::ResolvedAt))
                    .col(string_len_null(WorkOrderItem::ResolutionDescription, 500))
                    .col(date_time(WorkOrderItem::CreatedAt).null())
                    .col(string_len(WorkOrderItem::CreatedBy, 50).null())
                    .col(date_time(WorkOrderItem::UpdatedAt).null())
                    .col(string_len(WorkOrderItem::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_work_order_item_work_order")
                    .table(WorkOrderItem::Table)
                    .col(WorkOrderItem::WorkOrderId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(WorkOrderItem::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum WorkOrderItem {
    Table,
    Id,
    Uuid,
    TenantId,
    WorkOrderId,
    Description,
    ItemType,
    Status,
    Observation,
    ResolvedBy,
    ResolvedAt,
    ResolutionDescription,
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
enum WorkOrder {
    Table,
    Id,
}
