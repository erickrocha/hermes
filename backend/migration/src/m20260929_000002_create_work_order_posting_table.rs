use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-MT-02-S01` (`HRMS-705`, `C-026`): costed postings against a work
/// order. `part_id` is a real FK -- see `entity::work_order_posting_entity`'s
/// doc comment.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(WorkOrderPosting::Table)
                    .if_not_exists()
                    .col(pk_auto(WorkOrderPosting::Id).integer())
                    .col(binary_len_uniq(WorkOrderPosting::Uuid, 16))
                    .col(integer(WorkOrderPosting::TenantId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_work_order_posting_tenant")
                            .from(WorkOrderPosting::Table, WorkOrderPosting::TenantId)
                            .to(Tenant::Table, Tenant::Id),
                    )
                    .col(big_integer(WorkOrderPosting::WorkOrderId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_work_order_posting_work_order")
                            .from(WorkOrderPosting::Table, WorkOrderPosting::WorkOrderId)
                            .to(WorkOrder::Table, WorkOrder::Id),
                    )
                    .col(big_integer_null(WorkOrderPosting::WorkOrderItemId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_work_order_posting_work_order_item")
                            .from(WorkOrderPosting::Table, WorkOrderPosting::WorkOrderItemId)
                            .to(WorkOrderItem::Table, WorkOrderItem::Id),
                    )
                    .col(big_integer(WorkOrderPosting::PartId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_work_order_posting_part")
                            .from(WorkOrderPosting::Table, WorkOrderPosting::PartId)
                            .to(Part::Table, Part::Id),
                    )
                    .col(double(WorkOrderPosting::Quantity).not_null())
                    .col(big_integer(WorkOrderPosting::UnitValueCents).not_null())
                    .col(big_integer(WorkOrderPosting::TotalValueCents).not_null())
                    .col(string_len(WorkOrderPosting::CostSource, 30).not_null())
                    .col(date_time(WorkOrderPosting::CreatedAt).null())
                    .col(string_len(WorkOrderPosting::CreatedBy, 50).null())
                    .col(date_time(WorkOrderPosting::UpdatedAt).null())
                    .col(string_len(WorkOrderPosting::UpdatedBy, 50).null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_work_order_posting_work_order")
                    .table(WorkOrderPosting::Table)
                    .col(WorkOrderPosting::WorkOrderId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(WorkOrderPosting::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum WorkOrderPosting {
    Table,
    Id,
    Uuid,
    TenantId,
    WorkOrderId,
    WorkOrderItemId,
    PartId,
    Quantity,
    UnitValueCents,
    TotalValueCents,
    CostSource,
    CreatedAt,
    CreatedBy,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum WorkOrder {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum WorkOrderItem {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Part {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Tenant {
    Table,
    Id,
}
